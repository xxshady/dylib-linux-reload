#!/bin/bash

# Original command
original_command="heaptrack --use-inject target/debug/dylib-playground"

# Capture output of original command
output=$(eval "$original_command")

# Pipe output to grep and capture result
selected_line=$(echo "$output" | grep -m 1 "heaptrack --analyze")

if [ -n "$selected_line" ]; then
    # Execute selected line as command
    eval "$selected_line"
else
    echo "No matching line found."
fi
