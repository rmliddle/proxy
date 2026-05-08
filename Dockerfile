FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
ENV OPENSSL_STATIC=1
WORKDIR /app
COPY . .
RUN cargo build --release --bin proxy

FROM alpine:latest
RUN apk add --no-cache libgcc
COPY --from=builder /app/target/release/proxy .
ENTRYPOINT ["/proxy"]