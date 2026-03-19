# Block Hash Test Vectors

## Purpose
Define raw byte blocks and their expected block identifiers.

## Algorithms
- Hash: BLAKE3
- block_id = BLAKE3(block_bytes)

## Vector 0001: Empty block
- bytes: empty
- expected block_id (hex): PLACEHOLDER

## Vector 0002: ASCII "abc"
- bytes (hex): 616263
- expected block_id (hex): PLACEHOLDER

## Vector 0003: 1024 bytes of 0x00
- bytes: 1024 zero bytes
- expected block_id (hex): PLACEHOLDER
