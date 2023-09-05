# left4proxy

A simple Layer-4 tcp forwarding solution that I couldn't do with haproxy... so I thought I could do it in rust for fun.

## What does it do?

When receiving a connection, the program will try a few destinations in order, whichever is available will get the payload and its response will be returned to the source.

## How good is it?

It's well-tested, I'd say. You can see how elaborate the tests are for such a simple program.

I've been using it for a long time for relaying in my networks. Super fast and reliable. Zero issues.
