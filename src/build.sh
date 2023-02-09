#!/bin/bash

exec cargo build -r
exec mv ../target/release/snip /usr/bin
