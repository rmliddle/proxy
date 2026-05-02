FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --bin database

FROM alpine:latest
RUN apk add --no-cache libgcc
COPY --from=builder /app/target/release/database .
ENTRYPOINT ["/database"]