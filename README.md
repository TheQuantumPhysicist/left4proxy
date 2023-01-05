# left4proxy

A simple Layer-4 tcp forwarding solution that I couldn't implement with haproxy... so I thought I could do it in rust for fun.

When receiving a connection, the program will try a few destinations in order, whichever is available will get the payload and its response will returned to the connecting party.

Quick and dirty. Basic tests are there and it works fine.
