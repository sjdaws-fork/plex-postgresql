# Dockerfile for plex-postgresql
# Build with Alpine 3.15 which has musl 1.2.2 - same as Plex's bundled musl!

FROM alpine:3.15 AS builder

# Install build dependencies
RUN apk add --no-cache \
    build-base \
    sqlite-dev \
    linux-headers \
    curl \
    perl

# Verify musl version matches Plex (1.2.2)
RUN /lib/ld-musl-*.so.1 --version 2>&1 | head -2

WORKDIR /build

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy source files
COPY src/ src/
COPY include/ include/
COPY rust/ rust/
COPY scripts/docker-build-shim.sh scripts/docker-build-shim.sh

# Build PostgreSQL/libpq, Rust core, shim, and collect runtime libs in /libs
RUN sh scripts/docker-build-shim.sh

# Runtime stage
FROM linuxserver/plex:latest

# Install PostgreSQL client for health checks, sqlite3 for schema fixes,
# python3 for data migration, gdb for debugging
RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-client \
    sqlite3 \
    python3 \
    gdb \
    && rm -rf /var/lib/apt/lists/*

# NOTE: Do NOT set LANG/LC_ALL/CHARSET here — Plex's bundled musl+boost::locale
# handles locale internally. Setting these can interfere with exception handling.

RUN mkdir -p /usr/local/lib/plex-postgresql

# Create symlinks for musl compatibility (architecture-specific)
# Our shim was built with Alpine which expects libc.musl-{arch}.so.1
# but Plex bundles musl as libc.so
RUN ARCH=$(uname -m) && \
    echo "Creating musl symlink for architecture: $ARCH" && \
    if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then \
        MUSL_ARCH="aarch64"; \
    elif [ "$ARCH" = "x86_64" ]; then \
        MUSL_ARCH="x86_64"; \
    else \
        echo "Warning: Unknown architecture $ARCH, using $ARCH as-is"; \
        MUSL_ARCH="$ARCH"; \
    fi && \
    ln -sf /usr/lib/plexmediaserver/lib/libc.so /usr/local/lib/plex-postgresql/libc.musl-${MUSL_ARCH}.so.1 && \
    echo "Created symlink: libc.musl-${MUSL_ARCH}.so.1 -> /usr/lib/plexmediaserver/lib/libc.so"

COPY --from=builder /libs/*.so* /usr/local/lib/plex-postgresql/

COPY schema/plex_schema.sql /usr/local/lib/plex-postgresql/
COPY schema/sqlite_schema.sql /usr/local/lib/plex-postgresql/
COPY schema/sqlite_column_types.sql /usr/local/lib/plex-postgresql/
COPY scripts/migrate_lib.sh /usr/local/lib/plex-postgresql/
COPY scripts/migrate_table.py /usr/local/lib/plex-postgresql/

# Copy the initialization script for s6-overlay
# This will run BEFORE Plex starts as part of the init sequence
COPY scripts/docker-entrypoint.sh /usr/local/lib/plex-postgresql/docker-entrypoint.sh
RUN chmod +x /usr/local/lib/plex-postgresql/docker-entrypoint.sh

# Create s6-overlay init script to run our initialization
RUN mkdir -p /etc/s6-overlay/s6-rc.d/init-plex-postgresql && \
    echo "oneshot" > /etc/s6-overlay/s6-rc.d/init-plex-postgresql/type && \
    echo "/usr/local/lib/plex-postgresql/docker-entrypoint.sh" > /etc/s6-overlay/s6-rc.d/init-plex-postgresql/up && \
    chmod +x /etc/s6-overlay/s6-rc.d/init-plex-postgresql/up && \
    mkdir -p /etc/s6-overlay/s6-rc.d/user/contents.d && \
    touch /etc/s6-overlay/s6-rc.d/user/contents.d/init-plex-postgresql && \
    mkdir -p /etc/s6-overlay/s6-rc.d/svc-plex/dependencies.d && \
    touch /etc/s6-overlay/s6-rc.d/svc-plex/dependencies.d/init-plex-postgresql

# Fix claim script: inject LD_PRELOAD into the temporary Plex start during claim
# The base image's init-plex-claim starts Plex without our shim, which crashes
# because the SQLite shadow DB has no schema. We patch it to use the shim.
RUN if [ -f /etc/s6-overlay/s6-rc.d/init-plex-claim/run ]; then \
        sed -i 's|LD_LIBRARY_PATH=/usr/lib/plexmediaserver:/usr/lib/plexmediaserver/lib|LD_PRELOAD=/usr/local/lib/plex-postgresql/db_interpose_pg.so LD_LIBRARY_PATH=/usr/local/lib/plex-postgresql:/usr/lib/plexmediaserver:/usr/lib/plexmediaserver/lib|' \
            /etc/s6-overlay/s6-rc.d/init-plex-claim/run && \
        mkdir -p /etc/s6-overlay/s6-rc.d/init-plex-claim/dependencies.d && \
        touch /etc/s6-overlay/s6-rc.d/init-plex-claim/dependencies.d/init-plex-postgresql && \
        echo "Patched init-plex-claim for PostgreSQL shim"; \
    fi

# Replace CrashUploader with no-op to prevent SIGCHLD crashes
# When CrashUploader exits, it sends SIGCHLD to Plex. With LD_PRELOAD active,
# libpq's pqsignal() interferes with Plex's signal handling, causing
# "Received unexpected async signal 17" crashes.
RUN mv /usr/lib/plexmediaserver/CrashUploader /usr/lib/plexmediaserver/CrashUploader.real 2>/dev/null || true && \
    printf '#!/bin/sh\nexit 0\n' > /usr/lib/plexmediaserver/CrashUploader && \
    chmod +x /usr/lib/plexmediaserver/CrashUploader

# Modify Plex run script at BUILD TIME to inject LD_PRELOAD
# This must be done at build time because s6-rc compiles services before oneshots run
# We use a heredoc approach via a temp file since sed multiline is tricky in Dockerfile
RUN SHIM_INJECT='# PostgreSQL shim injection\nexport LD_LIBRARY_PATH="/usr/local/lib/plex-postgresql:/usr/lib/plexmediaserver/lib:$LD_LIBRARY_PATH"\nexport LD_PRELOAD="/usr/local/lib/plex-postgresql/db_interpose_pg.so"' && \
    sed -i "2i\\${SHIM_INJECT}" /etc/s6-overlay/s6-rc.d/svc-plex/run && \
    cat /etc/s6-overlay/s6-rc.d/svc-plex/run
