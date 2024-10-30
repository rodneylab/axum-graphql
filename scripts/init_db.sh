#!/usr/bin/env bash

set -x
set -eo pipefail

if ! [ -x "$(command -v sqlite3)" ]; then
	echo >&2 "Error: sqlite3 is not installed."
	exit 1
fi

if ! [ -x "$(command -v sqlx)" ]; then
	echo >&2 "Error: sqlx is not installed."
	echo >&2 "Use:"
	echo >&2 "  cargo install --version='~0.8' sqlx-cli \
        --no-default-features --features runtime-tokio-rustls,postgres"
	echo >&2 "to install it."
	exit 1
fi

DATABASE_URL=sqlite://sqlite.db
export DATABASE_URL
sqlx database create
sqlx migrate run

>&2 echo "SQLite has been migrated, ready to go!"
