# build: docker build -f crawler/Dockerfile -t void-crawler .
# the build context must be the repo root so crawler/ files resolve
FROM docker.io/library/rust:1-bookworm AS builder
WORKDIR /src
# deps layer caching via mtimes is unreliable with oci copies - just build
COPY crawler/Cargo.toml crawler/Cargo.lock ./
COPY crawler/seeds.txt seeds.txt
COPY crawler/src src
COPY crawler/tests tests
RUN cargo build --release && strip target/release/void-crawler

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 void && mkdir -p /data && chown void /data
COPY --from=builder /src/target/release/void-crawler /usr/local/bin/void-crawler
USER void
VOLUME /data
ENV RUST_LOG=info
# subcommands: crawl / serve / run / stats / fetch / export / import
ENTRYPOINT ["void-crawler", "-c", "/data/crawler.toml"]
CMD ["run"]
