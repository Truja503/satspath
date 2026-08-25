# SatsPath v2: Witness Deployment Guide

## Overview

A SatsPath Transparency Witness is a lightweight, stateless-by-design node that protects against split-view attacks. It acts as an independent auditor for Transparency Log operators, ensuring they do not equivocate by presenting different valid trees to different clients.

## Deployment Topology

Witnesses MUST be deployed in separate administrative and cryptographic domains from the log operators they audit. A typical deployment topology includes:

1. **The Log Operator**: Hosts the primary Transparency Log and resolves identities.
2. **Independent Witnesses (N)**: Operated by different entities across diverse infrastructure providers (e.g. one on AWS, one on GCP, one self-hosted).
3. **The Resolving Client**: Enforces a `K-of-N` witness policy defined in the namespace's DNS descriptor.

## Split-View Detection

If a log operator equivocates, it must present two different `root_hash` values for the same `log_id` and `tree_size`. When a witness encounters a checkpoint whose size matches its pinned state but the root differs, it halts and persistently records the permanent evidence of cryptographic failure. The witness will never sign a checkpoint for that log again until a manual operator-key rotation resets trust.

## Security Guarantees

- **Non-equivocation**: As long as at least one witness in the threshold set is honest, an operator cannot conduct a split-view attack without generating permanent, cryptographic proof of fraud.
- **No Liveness Requirement**: Witnesses are not required to be online for every resolution. They only need to be reachable by the operator when a new checkpoint is published.
