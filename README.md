# left4proxy

A simple Layer-4 tcp forwarding solution that I couldn't do with haproxy... so I thought I could do it in rust for fun.

## What does it do?

When receiving a connection, the program will try a few destinations in order, whichever is available will get the payload and its response will be returned to the source.

## Usage

You can run it immediately from cargo. No need to have an executable built and shipped.

Let's say that you want to forward all traffic from port 80 in localhost to port 1.2.3.4:8080, but if 1.2.3.4:8080 is not available, then forward to 1.2.3.4:8081, and if that's not available, then forward to 1.2.3.4:8082.

```bash
$ left4proxy 127.0.0.1:80 1.2.3.4:8080 1.2.3.4:8081 1.2.3.4:8082
```

## But how is it different than HAProxy?

The problem with HAProxy in this scenario is that it will put big emphasis on the availability of the destinations all the time. On the other hand, left4proxy doesn't care. When it receives a connection, ONLY then, it'll start attempting the connections.

## How good is it?

It's well-tested, I'd say. You can see how elaborate the tests are for such a simple program.

I've been using it for a long time for relaying in my networks. Super fast and reliable. Zero issues.

## Docker container?
Nope. Sorry. Not yet.
