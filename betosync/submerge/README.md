# Submerge

A data-consistency library for sharing data between multiple devices that ensures causal consistency while being always available using state-based CRDTs.

## Structure

```
submerge/
├── distributed_time
│       A low level library implementing vector clocks and hybrid logical clocks
│       for time tracking.
├── crdt
│       Implementation of state-based CRDTs, including Map, Set, Register, and
│       VectorData. These data types ensure eventual consistency and defines a
│       conflict resolution mechanism even when there are concurrent
│       modifications.
├── submerge
│       Main API surface implementing the data model, with interfaces to
│       integrate with network and storage. This layer is responsible for
│       serializing and deserializing the CRDTs, and also keeps track of the
│       versions at the document level.
├── submerge_java
│       Java bindings for submerge, including both the `submerge_java` crate
│       that exposes the JNI functions, and the Java side library
│       `com.google.android.submerge` that exposes an idiomatic Java API.
└── submerge_internal_proto
        An internal crate to support proto serialization and deserialization.
```

## Why is it called submerge?

1. it is designed for ~~sinking~~ syncing
2. its core operation is the `merge` function

## Why use submerge?

When syncing data between multiple devices that can get connected and disconnected, and where the network topology can change, it is often difficult to ensure that all of the devices converge to the same consistent view of the data. Naive approaches like wall-clock-based last-writer-wins may not reflect the programmer's or user's intention and may result in data loss.

Submerge builds upon the research done on CRDT and eventually convergent data types and utilizes state-based CRDTs to achieve the eventual consistency guarantees. It uses Hybrid Logical Clocks for time-tracking to ensure that causality is prioritized over wall-clock time (which are often out-of-sync between one another).

Submerge combines the theoretical foundations with generative fuzz-based testing to ensure that the asserted invariants are being upheld by running through millions of generated test cases.
