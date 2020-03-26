# Gatekeeper

A [SOCKS5] Server written in Rust.

## Features
### Authentication Method

Any authentication method is not supported.

The client connects to the server is required for sending `X'00'` (`NO AUTHENTICATION REQUIRED`) as a method selection message.

### Command

Only `CONNECT` command is supported.

### Filter

Gatekeeper allow users to restricting connection based on:

- target address
    - ip address
    - domain name (regex matching)
- port number
- protocol (currently, tcp is only supported)


## Install

## How to use


[SOCKS5]: ftp://ftp.rfc-editor.org/in-notes/rfc1928.txt "SOCKS Protocol Version 5"
