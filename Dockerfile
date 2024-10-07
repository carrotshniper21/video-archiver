FROM rust:latest as builder

WORKDIR /usr/src/app
COPY . .
RUN  cargo build --release && mv ./target/release/video-archiver ./video-archiver
CMD ./video-archiver
