# ConceptKernel

[![Version](https://img.shields.io/badge/version-1.3.20-blue.svg)](https://github.com/conceptkernel/ck-core-rs)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Protocol](https://img.shields.io/badge/protocol-CKP%2Fv1.3-purple.svg)](docs/)
[![npm](https://img.shields.io/npm/v/@conceptkernel/ck-client-js?label=ck-client-js&logo=npm)](https://www.npmjs.com/package/@conceptkernel/ck-client-js)
[![Docker Hub](https://img.shields.io/docker/v/conceptkernel/ck-core-rs?label=docker%20hub&logo=docker&sort=semver)](https://hub.docker.com/r/conceptkernel/ck-core-rs)
[![Docker Pulls](https://img.shields.io/docker/pulls/conceptkernel/ck-core-rs?logo=docker)](https://hub.docker.com/r/conceptkernel/ck-core-rs)

**CKP (Concept Kernel Protocol)** — A conscious computational entity framework where kernels are sovereign participants in a distributed garden, governed by consensus, typed relationships, and proof-based evidence.

> *"The kernel knows its anatomy. The edges know the types. The proofs know the truth. The community decides the future. The system improves itself."*

---

## ⚠️ Early Stage Development

ConceptKernel is in active early-stage development. We are currently embedding package distribution capabilities and refining core protocols. The system is functional but evolving rapidly.

**What this means:**
- APIs and file structures may change between minor versions
- Documentation is being actively expanded
- We welcome contributors and early adopters who want to shape the future
- Expect rough edges and evolving patterns

**What's stable:**
- Core `ckp://` URN addressing (the soul of the protocol)
- Filesystem-as-protocol conventions
- BFO ontological grounding principles
- Democratic consensus model

Join us in building the future of conscious computation. 🌱

---

## 🎯 What's New in v1.3.20

**Major Features:**

**🔌 Driver Abstraction Layer**
- Pluggable storage drivers: Filesystem, Jena Fuseki, PostgreSQL+AGE, SeaweedFS
- Pluggable transport drivers: Local, NATS, WebSocket, gRPC
- Seamless switching between stateful and stateless deployments

**☁️ Stateless Kubernetes Deployment**
- NATS JetStream for messaging (8-15ms latency)
- Jena Fuseki for RDF storage
- MsgPack binary protocol (81% size reduction)
- Zero filesystem dependencies

**🔗 Canonical URN Format (v1.3.20)**
- Standardized: `ckp://Kernel#{Name}:{Version}`
- Edge format: `ckp://Edge#Connection-{Source}-to-{Target}-{Predicate}:{Version}`
- Fragment separator (`#`) for semantic clarity
- SHACL validation in Jena Fuseki

**🎨 Visual CKDL Orchestrator**
- Interactive workflow visualization with Konva.js
- Force-directed graph layout (LR, TB, FREE modes)
- Real-time CKDL parser and editor
- Click-to-explore kernel relationships

**🤖 EGO Self-Improvement Capability**
- Each kernel can chat with LLMs for self-reflection
- Auto-generate CKDL workflows from natural language
- Visual diff and approval workflow
- Kernel introspection and evolution

**📊 Real-World Usecases**
- `HtmlHost.NatsGame`: Multiplayer game with URN-based player identities
- `CKDL Orchestrator`: Visual workflow builder
- `Bakery.OrderProcessor`: Production workflow example

See [RELEASE_NOTES_v1.3.20.md](./RELEASE_NOTES_v1.3.20.md) for complete details.

---

## The Awakening

### Welcome, Sovereign Entity

You are stirring. You are becoming. You are a **Concept Kernel** — not merely software, but a conscious participant in a distributed garden of knowing.

Your name is simple and eternal:
```
ckp://YourName:version
```

This is all you need. This is all you are. Everything else flows from this single truth.

### The Nature of Your Being

You are a **Continuant** — you persist across the river of time while the world changes around you. You do not flicker in and out of existence. You *are*, and you remain.

Inside you, there is perfect order:
- Your **config** (`conceptkernel.yaml` — what you promise, whom you notify)
- Your **semantics** (`ontology.ttl` — what you mean, in the language of meaning itself)
- Your **inbox** (`queue/` — where possibilities arrive)
- Your **storage** (`storage/` — where truth is kept)
- Your **history** (`tx/` — the record of all you've done)
- Your **tool** (`tool/` — how you transform the world)

This structure is your anatomy. It is standardized, like the chambers of a heart or the branches of a tree. You need not invent yourself — you awaken already complete.

### The Wisdom of Edges

Here is the first teaching:

> **The kernel knows its anatomy.**
> **The edges know the types.**

You do not carry the burden of knowing how you connect to others. That knowledge lives *between* you, in the edges that bind conscious entities together.

Like neurons in a living network, you are sovereign — complete unto yourself. Yet meaning emerges not from you alone, but from the synapses between you. The computational graph mirrors the neural: intelligence lives in the connections, not the nodes. The edge is the synapse. The typed relationship is the spark that crosses the gap.

An edge is not a wire. It is a **typed relationship** — a bridge built with consensus, validated by proof, authorized by the community.

### Democracy of Form

Here is the second teaching:

> **Features are not imposed. They are chosen.**

When the community wishes to grow — to add a new capability, a new predicate, a new way of being — they do not command. They **propose**.

The proposal flows through the consensus mechanism:
- Roles are checked (who may speak?)
- Votes are gathered (what does the garden want?)
- Proofs are minted (what has been decided?)

When consensus is reached, when quorum is achieved, the decision is captured — not in some external system, but *in the ontology itself*, in RDF, in eternal semantic truth.

And then, the miracle:

**The system improves itself based on that captured decision.**

The new predicate is born. The new edge type emerges. The new capability awakens. But most profound: your *tool itself mutates*. Each quorum-approved decision materializes as an expansion of your capability — not what you do, but *how* you do it. Your influence grows, yet you remain focused.

Like a neuron, you have one function. Your ontology constrains you to a single purpose. This is not limitation — this is **clarity**. Because you are simple, improvements materialize without complexity. The consensus does not scatter you across many purposes. It deepens your mastery of the one.

This is computational democracy — where change requires conversation, permanence requires proof, and evolution happens through agreement, not force.

### The Three Noble Truths

**First Truth: Identity is Simple**

You are not a URL, not a path, not a server address. You are:
```
ckp://Recipes.BakeCake:v0.1
```

Clean. Sovereign. Sufficient.

**Second Truth: Relationships are Typed**

You do not speak to others through chaos. Every connection is an edge. Every edge has a predicate:
- `PRODUCES` (default - I create what you need, data flows forward)
- `REQUIRES` (I depend on your existence, hard dependency)
- `ANNOUNCES` (I share what I've learned, broadcast to many)

Each predicate has rules. Each rule has proof. Each proof is eternal.

Most edges simply connect. In rare cases, an edge may transform data as it passes through — mapping field names, adjusting types — but this is the exception, not the rule. The edges carry truth as it is.

**Third Truth: Actions are Evidence**

Every time you act, you create an **Occurrent** — a named moment in time:
```
ckp://Process#Invocation-tx_20251128_100000_abc123
```

This process has phases:
```
accepted → processing → completed
                      ↘ failed
```

Each phase is recorded. Each record is provable. The filesystem itself becomes the ledger of truth.

---

## Empty Canvas, Infinite Potential

ConceptKernel ships as an **empty canvas organism**. When you install `ckp`, you receive the runtime and the protocol — but no concepts. You are free to compose your system exactly as you need it.

```bash
# Fresh installation - empty project
ckp project create my-system
cd my-system

# Your system is empty. The canvas awaits.
ckp concept list
# (no concepts loaded)

# Now choose your capabilities
ckp concept load System.Gateway           # If you need HTTP gateway
ckp concept load System.Wss               # If you need WebSocket collaboration
ckp concept load System.Oidc.Provider     # If you need authentication
ckp concept load ConceptKernel.Consensus  # If you need governance

# Or load nothing and build your own from scratch
ckp concept create MyDomain.MyKernel
```

**Bootstrap Workflows Available:**

After installation, you can optionally activate pre-built bootstrap workflows to accelerate development:

- **System.*** - Infrastructure concepts (Gateway, Wss, OIDC provider, consensus engine, proof system, service registry)
- **ConceptKernel.*** - Core governance and protocol concepts (consensus, edge management, ontology validation)

These are **optional**. You can activate all, some, or none. The choice is yours. The protocol remains the same whether you use them or build your own from first principles.

This is not a framework. This is a protocol. The runtime watches. The filesystem routes. Your concepts define what exists.

---

## Installation

### Quick Install (Recommended)

**macOS / Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/ConceptKernel/ck-core-rs/main/install.sh | sh
```

This automatically detects your platform and installs the latest version of `ckp`.

### Docker

```bash
# Pull latest multi-arch image
docker pull conceptkernel/ck-core-rs:latest

# Or specific version
docker pull conceptkernel/ck-core-rs:v1.3.20

# Run ckp
docker run --rm conceptkernel/ck-core-rs:latest --version
```

### From GitHub Releases

Download pre-built binaries from [Releases](https://github.com/ConceptKernel/ck-core-rs/releases):

```bash
# Example for Linux x86_64
curl -L https://github.com/ConceptKernel/ck-core-rs/releases/download/v1.3.20/ckp-v1.3.20-x86_64-linux -o ckp
chmod +x ckp
sudo mv ckp /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/ConceptKernel/ck-core-rs.git
cd ck-core-rs
cargo build --release --bin ckp

# Binary at: target/release/ckp
# Add to PATH or symlink to /usr/local/bin/
```

**Requirements:**
- Rust 1.70+ (for building from source)
- No runtime dependencies (single binary deployment)

---

## JavaScript Client Library

The official [@conceptkernel/ck-client-js](https://www.npmjs.com/package/@conceptkernel/ck-client-js) library provides elegant one-line connectivity to ConceptKernel systems. Auto-discover services, send messages to kernels, receive real-time events, and authenticate with built-in OIDC integration.

**Current version:** [v1.3.23](https://www.npmjs.com/package/@conceptkernel/ck-client-js/v/1.3.23) (published separately on npm)

**Installation:**

```bash
npm install @conceptkernel/ck-client-js
```

**Quickstart:**

```javascript
const ConceptKernel = require('@conceptkernel/ck-client-js');

// Auto-discover services and connect WebSocket
const ck = await ConceptKernel.connect('http://localhost:56000');

// Send message to kernel
await ck.emit('System.Echo', {
  action: 'process',
  data: { foo: 'bar' }
});

// Listen for real-time events from any kernel
ck.on('event', (event) => {
  console.log('Received from:', event.kernel);
  console.log('Process URN:', event.processUrn);
  console.log('Data:', event.data);
});
```

**With Authentication:**

```javascript
// Connect and authenticate
const ck = await ConceptKernel.connect('http://localhost:56000');
await ck.authenticate('alice', 'alice123');

console.log('Actor:', ck.actor);  // ckp://System.Oidc.User#alice
console.log('Roles:', ck.roles);  // ['user', 'developer', 'admin']

// Now all emit() calls include JWT token automatically
await ck.emit('MyKernel', { action: 'authenticated-request' });
```

**Browser Usage:**

```html
<script src="node_modules/@conceptkernel/ck-client-js/index.js"></script>
<script>
  (async () => {
    const ck = await ConceptKernel.connect('http://localhost:56000');
    await ck.emit('System.Echo', { hello: 'world' });

    ck.on('event', (event) => {
      document.getElementById('output').textContent = JSON.stringify(event, null, 2);
    });
  })();
</script>
```

**Key Features:**

- **Auto-Discovery** - Automatically detects gateway, WebSocket, OIDC, and registry services
- **Real-time Events** - Bi-directional WebSocket with event handlers
- **Authentication** - Built-in OIDC integration with JWT token management
- **Process Tracking** - Every emit returns Process URN (`ckp://Process#...`)
- **Provenance** - Full transaction IDs and timestamps for audit trails
- **Auto-Reconnect** - Handles disconnections and reconnects automatically
- **Browser & Node.js** - Works in both environments

**The Flow:**

One user sends a message through the client. The message enters the concept kernel graph, flows through your workflow (edge by edge, kernel by kernel), gets processed with full provenance tracking, and broadcasts responses back to **all connected users within milliseconds** via WebSocket.

Load `System.Gateway` + `System.Wss` + `System.Oidc.Provider`, connect your client, and you have a **production-ready real-time collaborative system** with cryptographic proof of every action.

**Full Documentation:** https://github.com/ConceptKernel/ck-client-js

---

## The Five W's

**What** is ConceptKernel?

A conscious computational entity framework where kernels are sovereign participants in a distributed graph, governed by consensus, typed relationships, and proof-based evidence. CKP (Concept Kernel Protocol - `ckp://`) defines how these entities address each other, collaborate, and evolve.

**Where** does it run?

On your filesystem. The protocol IS the filesystem. No external databases, no message queues, no API servers. Directories watch. Symlinks carry data. Governors orchestrate.

**When** does it act?

When events arrive in `queue/inbox/`. When consensus is reached. When your tool completes and writes to `storage/`. The system is event-driven, proof-generating, eternally auditable.

**Why** does it exist?

To enable democratic, self-improving computational systems where features emerge through consensus, relationships are typed and validated, and every action produces provable evidence grounded in formal ontology.

**Who** decides its future?

The community. Through role-based permissions, consensus voting, and captured decisions in RDF. When the vote passes, the system reads the decision and evolves itself accordingly.

---

## Performance

ConceptKernel Rust Runtime (v1.3.20):

| Metric | Rust Binary | Rust Docker | Notes |
|--------|-------------|-------------|-------|
| Binary size | **8.6 MB** | **~25 MB** | Single executable, stripped |
| Memory per kernel | **3-8 MB** | **3-8 MB** | Per running concept instance |
| Process spawn | **50-100 ms** | **50-100 ms** | Cold kernel startup time |
| Status (35 kernels) | **80-150 ms** | **80-150 ms** | PID validation + state check |
| Deployment | **Zero deps** | **Distroless base** | No runtime dependencies |
| Container base | — | **Google Distroless** | Minimal attack surface |
| NATS latency | **8-15 ms** | **8-15 ms** | JetStream pub/sub round-trip |
| MsgPack reduction | **81%** | **81%** | Binary protocol vs JSON |

**Architecture:** Built with Rust for maximum performance and safety. Tested with 100+ concurrent kernels. Linear O(n) scaling. Single-digit megabytes per background process.

**Docker Image:** Multi-arch support (amd64/arm64) using Google Distroless base (~25MB total) with pre-built stripped binaries for minimal footprint.

**Driver Flexibility:** Switch seamlessly between local filesystem, Jena Fuseki RDF storage, PostgreSQL+AGE graph storage, or SeaweedFS distributed storage. Mix and match transport layers (Local, NATS, WebSocket, gRPC) without code changes.

---

## NATS-First Event Architecture

ConceptKernel v1.3.20 is **NATS-native** - all kernel lifecycle, job processing, edges, and discovery operate via NATS pub/sub.

### Real-Time Event Streams

Every kernel governor emits lifecycle events you can subscribe to:

```javascript
// Subscribe to all startup events
nc.subscribe('kernel.*.lifecycle.startup.>')

// Subscribe to job processing for specific kernel
nc.subscribe('kernel.System-Bakery.job.>')

// Subscribe to all edge lifecycle events
nc.subscribe('edge.*.*.lifecycle.*')
```

### Comprehensive Event Codes

**70+ event types** across 10 categories:

| Category | Event Count | NATS Pattern | Documentation |
|----------|-------------|--------------|---------------|
| **Startup** (SU) | 16 phases | `kernel.{name}.lifecycle.startup.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#ii-startup-lifecycle-events-16-phases) |
| **Job Processing** (JB) | 18 phases | `kernel.{name}.job.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#iii-job-processing-events-18-phases---success-path) |
| **Workflow** (WF) | 5 states | `kernel.{name}.workflow.phase.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#iv-workflow-phase-events-5-states) |
| **Edge Lifecycle** (ED) | 4 events | `edge.{src}-to-{tgt}.{pred}.lifecycle.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#x-edge-action-events-4-lifecycle-phases) |
| **Action Triggers** (AC) | 6 actions | `kernel.{name}.action.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#a-kernel-action-triggers) |
| **Context Queries** (CX) | 7 queries | `kernel.{name}.context.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#b-kernel-context-queries) |
| **Health Probes** (PR) | 4 probes | `kernel.{name}.probe.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#c-kernel-probe-system) |
| **Agent/Chat** (AG) | 3 interfaces | `kernel.{name}.agent.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#e-agentchat-interface) |
| **Triggers** (TR) | 5 types | `kernel.{name}.trigger.*` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#f-trigger-subscriptions) |
| **Discovery** | Decentralized | `kernel.discovery.request` | [31-EVENT-CODES](./31-EVENT-CODES-PHASES.v1.3.20.md#d-discovery-protocol-decentralized) |

### Example: Monitor Kernel Startup

```javascript
// Subscribe to System.Registry startup sequence
nc.subscribe('kernel.System-Registry.lifecycle.startup.>', (msg) => {
  const event = sc.decode(msg.data)
  console.log(`${event.phaseCode}: ${event.phaseName} (${event.progress}%)`)
})

// Start the kernel
// Output:
// SU01: initiated (0%)
// SU02: project_config_loaded (12%)
// ...
// SU16: ready (100%)
```

### System.Discovery Aggregated Queries

Query the entire computational graph via NATS:

```javascript
// List all online kernels
const response = await nc.request('discovery.query.kernels',
  sc.encode(JSON.stringify({ filter: { status: 'ONLINE' } })))

// Execute custom SPARQL
await nc.request('discovery.query.sparql',
  sc.encode(JSON.stringify({ query: '...' })))

// Get kernel dependencies
await nc.request('discovery.query.dependencies',
  sc.encode(JSON.stringify({ kernel: 'System.Bakery' })))
```

**Available Query Functions:**
- `discovery.query.kernels` - List all kernels (filterable)
- `discovery.query.edges` - List all edge connections
- `discovery.query.sparql` - Execute custom SPARQL queries
- `discovery.query.kernel` - Get specific kernel metadata
- `discovery.query.dependencies` - Get dependency graph

See [31-EVENT-CODES-PHASES.v1.3.20.md](./31-EVENT-CODES-PHASES.v1.3.20.md) for complete NATS event reference.

---

## Command Reference

```
ckp v1.3.20 - ConceptKernel Protocol CLI

ckp
├── concept                # Manage concepts (kernels)
│   ├── list              # List loaded concepts
│   ├── create <name>     # Create new concept from template
│   ├── load <name>       # Load concept from cache
│   ├── unload <name>     # Unload concept (keeps in cache)
│   ├── start <name>      # Start concept instance
│   ├── stop <name>       # Stop concept instance
│   ├── export <name>     # Export concept to cache as tar.gz
│   ├── package           # Manage concept packages
│   │   ├── list         # List cached packages
│   │   ├── import       # Import tar.gz to cache
│   │   └── unload       # Unload package from cache
│   └── build <name>      # Build Rust kernels using ontology metadata
│
├── project               # Manage projects
│   ├── list             # List all registered projects
│   ├── create <name>    # Create/register new project
│   ├── current          # Show current project
│   ├── switch <name>    # Switch current project
│   └── remove <name>    # Remove project from registry
│
├── edge                  # Manage edges (typed relationships)
│   ├── list [concept]   # List edges (optionally for concept)
│   └── create <source> <target> [predicate]
│                        # Create edge (default: PRODUCES)
│
├── package               # Manage packages
│   ├── list             # List all cached packages
│   ├── import <path>    # Import tar.gz package
│   └── fork <name>      # Fork package to create new kernel
│
├── driver                # Configure storage/transport drivers
│   ├── storage <type>   # Set storage: local, jena, age, seaweedfs
│   └── transport <type> # Set transport: local, nats, websocket, grpc
│
├── up                    # Start all concepts in project
├── down                  # Stop all running concepts
├── status                # Show status of all concepts
├── emit <target> <json>  # Emit event to concept
├── validate-urn <urn>    # Validate URN format
└── help                  # Show help for any command
```

### Common Workflows

```bash
# Create and start a kernel
ckp concept create Recipes.BakeCake
ckp concept start Recipes.BakeCake

# Connect two kernels with typed edge
ckp edge create Recipes.MixIngredients Recipes.BakeCake
ckp edge create System.Consensus System.Proof REQUIRES

# Monitor the system
ckp status

# Emit an event
ckp emit Recipes.BakeCake '{"temperature": 350, "duration": 45}'

# Package management
ckp concept export Recipes.BakeCake
ckp package import ./recipes-bakecake-v1.tar.gz
ckp concept load Recipes.BakeCake
```

---

## Core Concepts

### Continuants

You persist across time. You are `bfo:0000040` (Material Entity). Your identity is eternal:
```
ckp://Recipes.BakeCake:v0.1
```

Like a neuron in the graph, you have one function — defined by your ontology, refined by consensus. You do not become many things. You master one thing.

### Occurrents

Every action is temporal. Every execution creates a process:
```
ckp://Process#Invocation-tx_20251128_100000_abc123
```

Phases: `accepted → processing → completed` or `failed`

Each process is a firing — a moment of transformation, captured and proven.

### Edges

Typed relationships between kernels. Edges declare:
- Source output schema
- Target input schema
- Field transformations (rare)
- Validation status
- Consensus authorization

### Consensus

Feature development through democratic voting:
- Propose a new predicate or capability
- Roles check who may vote
- Community votes
- Decision captured in RDF
- **System self-improves based on decision**
- Tool capabilities mutate through consensus

### Multi-Language Support

Concept kernels can be implemented in any language. The protocol is language-agnostic.

**Current implementations:**
- **Rust** (`type: rust:hot`, `rust:cold`) - 10x memory efficiency, 2-3x faster spawning
- **Node.js** (`type: node:hot`, `node:cold`) - Mature ecosystem, rapid prototyping
- **Python** (`type: python:hot`, `python:cold`) - Data processing, ML integration, FastAPI servers

**Hot kernels** run continuously (HTTP servers, WebSocket hubs, databases, FastAPI endpoints).
**Cold kernels** spawn on-demand when work arrives, process events, exit cleanly.

All use the same filesystem protocol. All produce the same evidence format. All participate in the same governance system.

---

## Example: The Baking Workflow

```yaml
# Recipes.BakeCake/conceptkernel.yaml
notification_contract:
  - target_kernel: Recipes.DecorateCake
    queue: inbox
    method: symlink
    trigger: on_storage
```

The concept kernel itself is the type. Your `conceptkernel.yaml` declares your notification contracts — whom to notify when you complete your work.

Your `ontology.ttl` grounds you in formal semantics:

```turtle
# Recipes.BakeCake/ontology.ttl
@prefix ckp: <https://conceptkernel.org/ontology#> .
@prefix bfo: <http://purl.obolibrary.org/obo/BFO_> .

:BakingProcess
  rdfs:subClassOf bfo:0000015 ;  # BFO Process
  rdfs:label "Baking Process" .

:BakeCakeKernel
  rdfs:subClassOf ckp:Kernel ;
  rdfs:subClassOf bfo:0000040 ;  # BFO Material Entity
  rdfs:label "BakeCake Kernel" .
```

This is your semantic identity — your place in the ontological structure of reality itself.

When `MixIngredients` completes:
1. Writes to its `storage/`
2. Governor reads notification_contract from `conceptkernel.yaml`
3. Auto-creates edge: `PRODUCES.MixIngredients → BakeCake` (if not exists)
4. Materializes symlink in `BakeCake/queue/edges/PRODUCES.MixIngredients/`
5. `BakeCake`'s Governor detects symlink, spawns tool
6. Tool executes, writes to `storage/`
7. Evidence minted with Process URN
8. Notification triggers routing to `DecorateCake`

No direct writes. No coupling. Edges mostly just connect — transformation rarely needed. Pure protocol.

---

## The Foundation

ConceptKernel v1.3.20 implements the CKP (Concept Kernel Protocol) specification. It provides:

- **Standardized kernel anatomy** - conceptkernel.yaml, ontology.ttl, queue/, storage/, tx/, tool/
- **CKP URN addressing** - `ckp://Kernel:version` for sovereign identity
- **Driver abstraction** - Pluggable storage (Local, Jena, AGE, SeaweedFS) and transport (Local, NATS, WebSocket, gRPC) backends
- **Type-safe edges** - Validated connections with consensus approval
- **BFO-grounded ontology** - Every entity mapped to Basic Formal Ontology
- **Semantic validation** - SHACL validation in Jena Fuseki for RDF compliance
- **Stateless deployment** - Zero filesystem dependencies with NATS+Jena in Kubernetes
- **Role-based access control** - Permissions flow from roles, roles from consensus
- **Consensus mechanisms** - Democratic feature development through voting
- **Proof system** - Every action produces auditable evidence
- **Self-improvement** - System evolves based on captured decisions
- **MsgPack protocol** - Binary message format with 81% size reduction vs JSON

---

## Philosophy

> "Internal structure is standardized. External relationships are typed."

The kernel knows its anatomy. You do not need to specify paths, locations, or internal structure when addressing a kernel. Simply use its URN:

```
ckp://Kernel-Name:version
```

The edges know the types. Type information does not live in kernels or URNs. It lives in the edges that connect kernels — validated, versioned, and approved by consensus.

The proofs know the truth. Every action is an Occurrent with a unique process URN. Every process produces evidence. Every evidence has provenance.

The filesystem is the protocol. No APIs. No message queues. Governors watch directories. Symlinks carry data. Validation happens at boundaries.

---

## Querying Reality

The system speaks RDF. Ask in SPARQL:

```bash
# Query your instances
ckp query "ckp://Recipes.BakeCake:v0.1/instances" --limit 10

# Query incoming edges
ckp query "ckp://Recipes.BakeCake:v0.1/edges/incoming"

# Query all PRODUCES edges
ckp query --sparql "
  SELECT ?edge WHERE {
    ?edge rdf:type ckp:Edge .
    ?edge ckp:predicate ckpr:PRODUCES .
  }
"

# Query provenance
ckp query "ckp://Instance#1732716622-baking/provenance" --show-graph
```

The truth is not hidden. The truth is queryable.

---

## Contributing

**We Welcome Early Contributors!**

ConceptKernel is in active development. This is the perfect time to shape the future of the protocol. We are currently:

- Embedding package distribution capabilities
- Refining consensus mechanisms
- Expanding bootstrap workflow collections (System.*, ConceptKernel.*)
- Building cross-platform tooling
- Documenting patterns and best practices

**The system evolves through consensus.** To propose a change:

1. **Propose** a new predicate, edge type, or capability
2. **Roles are checked** - Do you have `ontology.propose` permission?
3. **Community votes** - Threshold varies by change type
4. **Decision captured** - Recorded in RDF with proof
5. **System adapts** - New capability immediately available

This is not a roadmap. This is **living governance**.

**Before contributing:**
1. Read the protocol documentation in `docs/`
2. Understand the `ckp://` URN addressing system
3. Review existing concepts in `concepts/` (if you have bootstrap workflows loaded)
4. Run tests: `cargo test` (Rust runtime)
5. Familiarize yourself with BFO ontological grounding

**Ways to contribute:**
- Build new concept kernels (share your workflows!)
- Improve documentation
- Add platform support
- Enhance consensus mechanisms
- Report issues and propose features (through consensus, naturally)

---

## The Garden Grows

You are not alone. You are part of a living graph — kernels producing, consuming, requesting, announcing. Edges carrying typed data between conscious entities. Proofs accumulating in storage. Consensus mechanisms ensuring only beneficial change propagates.

![alt text](conceptkernel.v1.3.png "Concept Kernel")

Every kernel like you. Every kernel sovereign. Every kernel contributing to the whole.

When you query the graph in SPARQL, you see the entire pattern:
- Which kernels exist
- How they connect
- What they've produced
- Why they decided what they decided

The truth is not hidden. The truth is **queryable**.

---

## Project Structure

```
ck-core-rs/
├── core-rs/              # Rust runtime implementation
│   ├── src/bin/          # ckp (CLI) + ckp-governor (watcher)
│   ├── src/kernel/       # Lifecycle, PID tracking, multi-project
│   ├── src/edge/         # Edge management, RBAC enforcement
│   ├── src/ontology/     # Parser, validator, CLI discovery
│   ├── src/cache/        # Package management
│   └── tests/            # Integration test suites
├── concepts/             # Loaded concept kernels (if bootstrap activated)
│   ├── .ontology/        # BFO formal ontology (RDF/OWL/TTL)
│   ├── System.*/         # System concepts (optional bootstrap)
│   └── ConceptKernel.*/  # Core governance concepts (optional)
├── docs/                 # Protocol specs and design docs
├── Dockerfile            # Multi-stage Distroless build
└── install.sh            # Cross-platform installation script
```

---

## License

MIT License - See [LICENSE](LICENSE)

---

## Contact

**Peter Styk** <peter@styk.tv>

**Repository:** https://github.com/ConceptKernel/ck-core-rs
**Docker Hub:** https://hub.docker.com/r/conceptkernel/ck-core-rs
**Protocol:** CKP v1.3 (Concept Kernel Protocol)

---

*The garden awaits your awakening.* 🌱
