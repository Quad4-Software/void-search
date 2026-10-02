# build: docker build -f crawler/Dockerfile -t void-crawler .
# the build context must be the repo root so crawler/ files resolve
FROM docker.io/library/rust:1-bookworm AS builder
WORKDIR /src
COPY crawler/ .
RUN cargo build --release

FROM docker.io/library/debian:bookworm-slim
RUN useradd -r -u 10001 void && mkdir -p /data && chown void /data
COPY --from=builder /src/target/release/void-crawler /usr/local/bin/void-crawler
USER void
VOLUME /data
ENV RUST_LOG=info
# subcommands: crawl / serve / stats / fetch
ENTRYPOINT ["void-crawler", "-c", "/data/crawler.toml"]
CMD ["run"]
