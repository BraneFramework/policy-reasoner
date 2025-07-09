#!/bin/bash
# RUN POSIX EXAMPLE.sh
#   by Tim Müller
# 
# Script for running the POSIX example.
# 
# This will set the user permissions for the Jedis file correctly.
# 


# Execute from the script's folder
cd "$(dirname "$0")"

# Read CLI
debug=""
for arg in $@; do
    if [[ "$arg" == "--debug" ]]; then
        debug="--debug"
    fi
done

# Set the permissions
chmod 640 ./tests/posix/files/jedis.csv || exit "$?"

# Run the script
cargo run --example posix --features file-logger,posix-reasoner,serde,workflow -- "$debug" ./tests/workflow/jedis.json --config ./tests/posix/jedis.json || exit "$?"
