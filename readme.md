# aculs rust

## ⚠️WARNING⚠️

**This repository is not yet ready for production use.**

A lot of this code is still being heavily worked and, is missing security checks and has not been reviewed.

Feel free to browse the codebase and create issues for any problem you see.

## What is this ?

This is my Playground for developing a production ready Svalin.

## Purpose

Svalin is suppoed to be the first open-source end-to-end encrypted remote managment software with a focus on small businesses, indiciduals and goverment agencies.
This codebase is still far from where I want it to be, but I have to start somewhere.

Svalin has the following guidelines:
- Be easy to setup and use
- Be secure by default
- Allow self-hosting

# Architecture

## Todo
- [X] Init Routine
- [ ] Login Routine
- [X] Unlock Routine
- [ ] Make Credentials use an Arc internally
- [ ] Unlock Error when password wrong
- [ ] Agent init / join
- [ ] E2E Encryption
- [ ] Rate Limiting
- [ ] Basic Permission System
- [ ] Basic Managment (Live)
- [ ] Remote Desktop
- [ ] Network Security System and Alerting
- [ ] IPv6 Support
- [ ] 

## Svalin Network Architecture

1. QUIC with Network Authentication and Path encryption
2. Routing through the Svalin Network
3. P2P Encryption
