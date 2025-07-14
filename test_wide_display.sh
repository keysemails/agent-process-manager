#!/bin/bash

echo "Testing with different terminal widths..."

# Test with narrow terminal (80 cols)
echo -e "\n=== 80 column terminal ==="
COLUMNS=80 ./target/debug/apm list

# Test with medium terminal (120 cols)
echo -e "\n=== 120 column terminal ==="
COLUMNS=120 ./target/debug/apm list

# Test with wide terminal (200 cols)
echo -e "\n=== 200 column terminal ==="
COLUMNS=200 ./target/debug/apm list