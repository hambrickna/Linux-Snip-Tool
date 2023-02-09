use png;
use std::process::Command;
use std::{fs::File, io::BufWriter};
use xcb::x;

fn main() -> xcb::Result<()> {
    //Connection to the x server
    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();

    //Unique Id's needed to communicate with the x server
    let gc: x::Gcontext = conn.generate_id();
    let pid: x::Pixmap = conn.generate_id();
    let cid: x::Cursor = conn.generate_id();
    let fid: x::Font = conn.generate_id();
    let window: x::Window = conn.generate_id();

    let width = screen.width_in_pixels();
    let height = screen.height_in_pixels();

    //Creating the window
    conn.send_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width,
        height,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        value_list: &[
            x::Cw::OverrideRedirect(true),
            x::Cw::EventMask(
                x::EventMask::BUTTON1_MOTION
                    | x::EventMask::BUTTON_PRESS
                    | x::EventMask::BUTTON_RELEASE
                    | x::EventMask::KEY_PRESS,
            ),
        ],
    });

    conn.flush()?;

    {
        //Open x window's predefined mouse cursor font
        conn.send_request(&x::OpenFont {
            fid,
            name: b"cursor",
        });

        conn.send_request(&x::CreateGlyphCursor {
            cid,
            source_font: fid,
            mask_font: fid,
            source_char: 30,
            mask_char: 31,
            fore_red: 0xFFFF,
            fore_green: 0xFFFF,
            fore_blue: 0xFFFF,
            back_red: 0,
            back_green: 0,
            back_blue: 0,
        });

        conn.send_request(&x::CloseFont { font: fid });

        conn.send_request(&x::ChangeWindowAttributes {
            window,
            value_list: &[x::Cw::Cursor(cid)],
        });
    }

    //Capture keyboard input
    conn.send_request(&x::GrabKeyboard {
        owner_events: true,
        grab_window: screen.root(),
        time: x::CURRENT_TIME,
        pointer_mode: x::GrabMode::Async,
        keyboard_mode: x::GrabMode::Async,
    });

    //Map the window to the screen.  Make it visible
    conn.send_request(&x::MapWindow { window });

    //Graphics context for xcb rendering
    conn.send_request(&x::CreateGc {
        cid: gc,
        drawable: x::Drawable::Window(window),
        value_list: &[x::Gc::Foreground(screen.white_pixel()), x::Gc::LineWidth(1)],
    });

    //Create a pixmap of the root window to be used for refreshing the screen
    conn.send_request(&x::CreatePixmap {
        pid,
        depth: screen.root_depth(),
        drawable: x::Drawable::Window(window),
        width,
        height,
    });

    conn.flush()?;

    //Copy the current window to a pixmap to be drawn/refreshed as needed
    conn.send_request(&x::CopyArea {
        src_drawable: x::Drawable::Window(window),
        dst_drawable: x::Drawable::Pixmap(pid),
        gc,
        src_x: 0,
        src_y: 0,
        dst_x: 0,
        dst_y: 0,
        width,
        height,
    });

    conn.flush()?;

    let mut rectangle: &mut [x::Rectangle] = &mut [x::Rectangle {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    }];

    let mut xcoord: i16 = 0;
    let mut ycoord: i16 = 0;

    //Main event loop  Waits for events and responds to them.
    loop {
        let event = match conn.wait_for_event() {
            Err(xcb::Error::Connection(err)) => {
                panic!("Unexpected I/O error: {}", err);
            }
            Err(xcb::Error::Protocol(err)) => {
                panic!("Unexpected protocol error: {:#?}", err);
            }
            Ok(event) => event,
        };

        match event {
            //On Mouse-Button press, capture x and y mouse position
            xcb::Event::X(x::Event::ButtonPress(ev)) => {
                rectangle[0].x = ev.event_x();
                rectangle[0].y = ev.event_y();

                xcoord = ev.event_x();
                ycoord = ev.event_y();
            }

            xcb::Event::X(x::Event::ButtonRelease(_ev)) => {
                //Refresh the screen to the saved pixmap of the window
                conn.send_request(&x::CopyArea {
                    src_drawable: x::Drawable::Pixmap(pid),
                    dst_drawable: x::Drawable::Window(window),
                    gc,
                    src_x: 0,
                    src_y: 0,
                    dst_x: 0,
                    dst_y: 0,
                    width,
                    height,
                });

                conn.flush()?;

                //Grab image from user-created rectangle
                let cookie = conn.send_request(&x::GetImage {
                    format: x::ImageFormat::ZPixmap,
                    drawable: x::Drawable::Pixmap(pid),
                    x: rectangle[0].x,
                    y: rectangle[0].y,
                    width: rectangle[0].width,
                    height: rectangle[0].height,
                    plane_mask: u32::MAX,
                });

                //Create a new png file with Rgb colortype and the correct sizes
                let file = File::create("clip.png").unwrap();
                let writer = BufWriter::new(file);
                let mut encoder =
                    png::Encoder::new(writer, rectangle[0].width as _, rectangle[0].height as _);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder
                    .write_header()
                    .expect("Failed to write image header");

                let reply = conn.wait_for_reply(cookie).unwrap();

                let src = reply.data();

                //Adjust image byte data to go from BGR to RGB
                let mut data =
                    vec![0; rectangle[0].width as usize * rectangle[0].height as usize * 3];
                for (src, dest) in src.chunks(4).zip(data.chunks_mut(3)) {
                    dest[0] = src[2]; //r
                    dest[1] = src[1]; //g
                    dest[2] = src[0]; //b
                }

                //Write adjusted image data to file "img.png"
                writer
                    .write_image_data(&data)
                    .expect("Failed to write image data.");

                conn.flush()?;

                //Move clip.png to tmp folder
                Command::new("/bin/mv")
                    .args(["clip.png", "/tmp/clip.png"])
                    .spawn()
                    .expect("Failed to mv clip.png to tmp folder");

                //Run xclip command to copy created png file to clipboard
                Command::new("/bin/xclip")
                    .args([
                        "-selection",
                        "clipboard",
                        "-t",
                        "image/png",
                        "-i",
                        "/tmp/clip.png",
                    ])
                    .spawn()
                    .expect("xclip command failed to run");

                //Break the loop and return an Ok Status for xcb Result
                break Ok(());
            }

            xcb::Event::X(x::Event::KeyPress(ev)) => {
                //If "q" is pressed, exit application
                if ev.detail() == 0x18 {
                    break Ok(()); //Break out of application loop and return an ok result
                }
            }

            //When user's mouse moves adjust rectangle coordinates, width and height
            xcb::Event::X(x::Event::MotionNotify(ev)) => {
                let drawable = x::Drawable::Window(window);

                rectangle[0].height = (ev.event_y() - rectangle[0].y) as u16;
                rectangle[0].width = (ev.event_x() - rectangle[0].x) as u16;

                //Handle reflection across x-axis
                if ev.event_x() < xcoord {
                    rectangle[0].x = ev.event_x();
                    rectangle[0].width = (xcoord - ev.event_x()) as u16;
                }

                //Handle reflection across y-axis
                if ev.event_y() < ycoord {
                    rectangle[0].y = ev.event_y();
                    rectangle[0].height = (ycoord - ev.event_y()) as u16;
                }

                //Refresh the window screen
                conn.send_request(&x::CopyArea {
                    src_drawable: x::Drawable::Pixmap(pid),
                    dst_drawable: x::Drawable::Window(window),
                    gc,
                    src_x: 0,
                    src_y: 0,
                    dst_x: 0,
                    dst_y: 0,
                    width,
                    height,
                });

                conn.flush()?;

                //Draw the user's rectangle that shapes the image
                conn.send_request(&x::PolyRectangle {
                    drawable,
                    gc,
                    rectangles: rectangle,
                });

                conn.flush()?;
            }
            _ => {} //If no event just continue looping
        }
    }
}
