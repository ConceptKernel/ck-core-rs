# ConceptKernel

[![Version](https://img.shields.io/badge/version-1.3.17-blue.svg)](https://github.com/conceptkernel/ckp)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Protocol](https://img.shields.io/badge/protocol-conceptkernel%2Fv1-purple.svg)](docs/CONCEPT-KERNEL.v1.3.16.md)

**A protocol-first, event-sourced system where background processes have a voice.**

ConceptKernel isn't just runtime infrastructure—it's a declaration of independence for autonomous processes. Every capability is a concept kernel that can extend the CLI, make decisions through consensus, prove its actions cryptographically, and coordinate with others through ontological contracts. The filesystem *is* the protocol. No external databases. No message queues. No coordination servers. Just concepts that know how to collaborate.

---

## The Architecture

You start with an empty project and a package manager. Everything else you compose from concept kernels—either existing ones from the registry or your own.

```bash
# Initialize empty project
ck project create my-system
cd my-system

# Project is empty—no concepts loaded yet
# Use the package manager to compose your system
ck concept load System.Gateway.HTTP
ck concept load System.WssHub
ck concept load System.Oidc.Provider

# Start your composed system
ck concept start System.Gateway.HTTP
```

Each concept kernel is:
- An **independent local git repository** with isolated data and software
- A **voice in the CLI** through dynamic command registration
- A **participant in governance** through consensus voting
- A **producer of cryptographic proof** for every action taken
- A **polyglot process** (Rust, Python, Node.js, any language)

When you backup, export, or upgrade a concept, **data stays isolated from software**. When you make critical changes, **quorum must be reached**. When decisions are recorded and tests pass, **software increments build and auto-release**—all independent, all auditable, all governed.

---

## Background Processes Have a Voice

Concept kernels declare CLI interfaces in their ontologies. The runtime discovers them dynamically and routes commands to the appropriate process.

**System.Oidc.User declares:**
```yaml
interfaces:
  - cli
  - notification
```

**Now you can:**
```bash
# Runtime discovers "user" command, routes to System.Oidc.User
ck user list
ck user create alice --roles developer,viewer
ck user show alice

# Each command emits an event to the concept's queue
# The concept processes it, produces proof, stores evidence
```

**System.Oidc.Role declares:**
```yaml
interfaces:
  - cli
  - notification
```

**Now you can:**
```bash
ck role list
ck role create kernel-developer --permissions kernel.create,kernel.modify
ck role assign alice kernel-developer
```

Every concept that exposes CLI gets discovered automatically. Type `ck` and see all loaded concepts offering commands. **The CLI grows as you load more concepts.** Background processes aren't hidden—they're first-class participants in your system's interface.

---

## Consensus-Driven Governance

Critical operations require consensus. System.Consensus manages proposals, collects votes, records decisions, and enforces outcomes.

```bash
# Alice proposes extending token expiry to 24 hours
ck emit System.Consensus '{
  "action": "propose",
  "policy": "token_expiry_24h",
  "duration": 24,
  "rationale": "Deploy environments need longer sessions"
}'

# Proposal created, stakeholders notified via System.WssHub
# Bob (with voting rights) reviews and votes
ck emit System.Consensus '{
  "action": "vote",
  "proposalId": "prop-a3f8b2",
  "vote": "approve"
}'

# Quorum reached (51% of system-admin + kernel-maintainer roles)
# Decision recorded with transaction ID and proof
# System.Oidc.Provider now honors 24h tokens with consensus proof attached
```

**What requires consensus:**
- Token expiry policy changes
- RBAC role modifications
- New kernel-to-kernel edge authorization
- Port range adjustments
- Any concept-declared governance point

Consensus isn't bureaucracy—it's **recorded proof that stakeholders agreed**. Every decision has a transaction ID. Every action references its authorization. Every upgrade carries governance provenance.

---

## Ontological Contracts & BFO Alignment

Concept kernels declare their capabilities through ontologies formally mapped to Basic Formal Ontology (BFO)—the gold standard for scientific provenance.

**System.Oidc.Provider ontology declares:**
```yaml
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: Org.ConceptKernel:System.Oidc.Provider
  description: OIDC Provider with JWT generation, governance-enforced token expiry
  type: rust:hot
  port: 3042

interfaces:
  - cli
  - http
  - notification

contracts:
  invocation:
    methods:
      - name: generate_token
        input: { username, password, clientType, requestedExpiry }
        output: { token, expiresAt, roles, permissions }
      - name: validate_token
      - name: refresh_token

governance:
  - Token expiry changes require System.Consensus approval
  - Default 1 hour, extensions need consensus proof
```

**BFO Mapping (concepts/.ontology/):**
- **Kernels** → `bfo:IndependentContinuant` (Material Entities)
- **Instances** → `bfo:GenericallyDependentContinuant` (Information)
- **Processes** → `bfo:Occurrent` (Temporal parts tracked)
- **Edges** → `bfo:RealizableEntity` (Relational dispositions)

65% BFO alignment achieved. Process URN tracking and Three Mediators integration (EdgeKernel, ConsensusKernel, WssHubKernel) in progress.

---

## File System IS the Protocol

No coordination required between runtimes. Two completely independent implementations (Node.js `ck` and Rust `ckr`) operate on the same filesystem without configuration, shared memory, or IPC.

**How:**
- Identical URN resolution → identical paths
- Identical symlink conventions for edges
- Identical JSONL transaction formats
- Identical PID:START_TIME validation (prevents PID reuse)
- Identical port slot allocation (100 ports per project)

**Example:**
```
concepts/
  .edges/                           # Edge relationship registry
    PRODUCES.MixIngredients/        # Edge directory (predicate.source)
      metadata.json                 # Edge metadata (URN, source, target, created)
    VALIDATES.Oidc.Provider/
      metadata.json

  System.Oidc.User/
    ontology.yaml                   # Contract declaration
    .git/                           # Independent git repository
    tool/                           # Runtime implementation (any language)
    queue/inbox/                    # Incoming events (filesystem queue)
    storage/                        # Evidence instances (JSONL + metadata)
    tx/                             # Transaction log
    logs/                           # Process logs
    .governor.pid                   # PID:START_TIME of watcher
    .tool.pid                       # PID:START_TIME of tool (hot kernels)
```

Node.js runtime starts a kernel. Rust runtime sees it running (PID file with start time). Both can emit events to its queue. Both can read its storage. **Zero coordination. Just filesystem conventions.**

### Edge Storage: concepts/.edges/

Edges are stored in `concepts/.edges/` as a centralized registry. Each edge gets its own directory named `{PREDICATE}.{Source}` containing metadata.

**Creating an edge:**
```bash
ck edge create PRODUCES Recipes.MixIngredients Recipes.BakeCake
```

**Creates:**
```
concepts/.edges/PRODUCES.Recipes.MixIngredients/
  metadata.json       # Edge metadata with URN, timestamps, governance proof
```

**metadata.json structure:**
```json
{
  "urn": "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.16",
  "predicate": "PRODUCES",
  "source": "Recipes.MixIngredients",
  "target": "Recipes.BakeCake",
  "version": "v1.3.16",
  "createdAt": "2025-11-27T12:30:00Z"
}
```

**Valid predicates:**
- `PRODUCES` - One concept produces output for another
- `NOTIFIES` - One concept notifies another of events
- `VALIDATES` - One concept validates another's output
- `TRIGGERS` - One concept triggers another's execution
- `REQUIRES` - One concept requires another to run first
- `LLM_ASSIST` - LLM-assisted transformation edge

When a concept emits an instance, the EdgeKernel reads `.edges/` to determine routing. If `PRODUCES.MixIngredients-to-BakeCake` exists, the instance gets routed to `Recipes.BakeCake/queue/inbox/` as a symlink.

**Edge routing flow:**
1. MixIngredients produces instance → `storage/1732845100-dough.inst/`
2. EdgeKernel scans `.edges/` for `PRODUCES.MixIngredients`
3. Finds target: `Recipes.BakeCake`
4. Creates symlink: `Recipes.BakeCake/queue/inbox/1732845100-dough.inst` → `../../Recipes.MixIngredients/storage/1732845100-dough.inst/`
5. BakeCake's governor detects new file in inbox, spawns tool, processes

**Zero message brokers. Zero databases. Just filesystem.**

---

## Multi-Language, Single Protocol

Concept kernels can be implemented in any language. The protocol is language-agnostic.

**Current implementations:**
- **Rust** (`type: rust:hot`, `rust:cold`) - 10x memory efficiency, 2-3x faster spawning
- **Node.js** (`type: node:hot`, `node:cold`) - Mature ecosystem, rapid prototyping
- **Python** (`type: python:cold`) - Data processing, ML integration

**Hot kernels** run continuously (HTTP servers, WebSocket hubs, databases).
**Cold kernels** spawn on-demand when work arrives, process events, exit cleanly.

All use the same filesystem protocol. All produce the same evidence format. All participate in the same governance system.

---

## Performance

**Rust Runtime vs Node.js v1.3.12:**

| Metric | Rust | Node.js | Improvement |
|--------|------|---------|-------------|
| Memory per kernel | 3-8 MB | 50-80 MB | **10x** |
| Process spawn | 50-100 ms | 150-300 ms | **2-3x** |
| Status (35 kernels) | 80-150 ms | 200-400 ms | **2.5x** |
| Binary size | 8 MB | 55 MB | **7x smaller** |
| Deployment | Single binary | Runtime + deps | **Zero deps** |

Tested with 100+ concurrent kernels. Linear O(n) scaling. Single-digit megabytes per background process.

---

## Package Management & Composition

The cache is your local registry. Remote registries are concept kernels that serve packages.

```bash
# List what's available in local cache
ck concept cache list

# Load a concept from cache to your project
ck concept load System.Gateway.HTTP --version v1.3.16

# Export a concept you built to cache
ck concept export MyDomain.MyKernel --version v0.1

# Import a .tar.gz from someone else
ck concept cache import ./their-kernel-v0.2.tar.gz

# Load it into your project
ck concept load TheirDomain.TheirKernel --version v0.2
```

Concepts are **packaged with version, architecture, and runtime** metadata:
```
System.Gateway.HTTP_v1.3.16_aarch64-darwin_rs.tar.gz
System.Gateway.HTTP_v1.3.16_x86_64-linux_rs.tar.gz
```

Load filters by platform automatically. Remote registries (themselves concept kernels) serve HTTP endpoints for package discovery and download.

**Data stays isolated.** When you export a concept, you can choose:
- Export software only (clean slate for others)
- Export software + data (backup/migration)
- Export software + sanitized data (demo instances)

When you upgrade, data migrates forward through version-aware transforms.

---

## Multi-Project Isolation

Run multiple ConceptKernel projects on the same machine. Each gets its own:
- **Port range** (slot-based allocation, 100 ports per project)
- **Process space** (full project path in process detection)
- **Evidence storage** (isolated storage/ directories)
- **Git repositories** (each concept is independent repo)

```bash
# Create project 1 (gets port slot 1: 56100-56199)
ck project create /home/alice/platform-api
cd /home/alice/platform-api
ck concept load System.Gateway.HTTP
ck concept start System.Gateway.HTTP  # Runs on 56101

# Create project 2 (gets port slot 2: 56200-56299)
ck project create /home/alice/ml-pipeline
cd /home/alice/ml-pipeline
ck concept load System.Gateway.HTTP
ck concept start System.Gateway.HTTP  # Runs on 56201

# Both run simultaneously, zero conflicts
```

Projects register at `~/.config/conceptkernel/registry`. Commands work from anywhere—runtime resolves via registry.

**Cross-project edges** are possible:
```bash
ck edge create PRODUCES ckp://platform-api/API.Orders ckp://ml-pipeline/ML.Forecasting
```

Platform API emits orders to ML pipeline for forecasting. Different projects, one protocol.

---

## CLI Commands

```bash
# Projects
ck project create [path]           # Initialize new project
ck project list                    # Show all registered projects
ck project current                 # Show current project
ck project switch <name>           # Switch active project

# Concepts (Package Management)
ck concept list                    # List loaded concepts in project
ck concept create <name>           # Create new concept from template
ck concept load <name>             # Load from cache to project
ck concept unload <name>           # Unload from project (stays in cache)
ck concept export <name>           # Export to cache as .tar.gz
ck concept cache list              # List cached packages
ck concept cache import <file>     # Import .tar.gz to cache

# Lifecycle
ck concept start <name>            # Start a concept instance
ck concept stop <name>             # Stop a concept instance
ck status                          # Show all running concepts
ck up                              # Start all concepts in project
ck down                            # Stop all concepts in project

# Edges (Typed Relationships)
ck edge create <predicate> <source> <target>
ck edge list [concept]             # List all edges or for specific concept

# Events
ck emit <target> '<json>'          # Emit event to concept queue

# Dynamic Commands (registered by loaded concepts)
ck user list                       # If System.Oidc.User is loaded
ck user create <name>
ck role list                       # If System.Oidc.Role is loaded
ck role assign <user> <role>
# ... and more as you load concepts
```

**The CLI grows with your system.** Load System.Consensus, get `ck consensus propose`. Load System.Proof, get `ck proof verify`. Background processes extend the interface.

---

## System Concepts

Pre-built concepts for common infrastructure:

**Identity & Authorization:**
- `System.Oidc.Provider` - JWT generation, OIDC flows, consensus-governed expiry
- `System.Oidc.User` - User management with CLI commands
- `System.Oidc.Role` - RBAC with role assignment and permission checking
- `System.Oidc.Token` - Token lifecycle management

**Governance & Proof:**
- `System.Consensus` - Proposal, voting, quorum, decision recording
- `System.Proof` - Cryptographic proof generation and verification

**Communication:**
- `System.Gateway.HTTP` - HTTP API gateway with OIDC integration
- `System.WssHub` - WebSocket collaboration hub for real-time broadcasts
- `System.Registry` - Service discovery and health checking

**Secrets:**
- `System.Sops` - Mozilla SOPS integration for encrypted configuration

---

## Example: Building a Governed Collaboration Platform

```bash
# Start with empty project
ck project create collab-platform
cd collab-platform

# Compose system from packages
ck concept load System.Gateway.HTTP
ck concept load System.WssHub
ck concept load System.Oidc.Provider
ck concept load System.Oidc.User
ck concept load System.Oidc.Role
ck concept load System.Consensus
ck concept load UI.PaintStream

# Start infrastructure
ck concept start System.Gateway.HTTP

# Create users
ck user create alice --roles system-admin,developer
ck user create bob --roles developer
ck user create carol --roles viewer

# Start collaborative canvas
ck concept start UI.PaintStream

# Alice proposes feature change requiring consensus
ck emit System.Consensus '{
  "action": "propose",
  "policy": "enable_paintstream_recording",
  "rationale": "Users want to save and replay sessions"
}'

# Bob votes approve
ck emit System.Consensus '{"action": "vote", "proposalId": "prop-xyz", "vote": "approve"}'

# Quorum reached, decision recorded
# Feature increment built, tested, auto-released with governance proof
```

You now have:
- HTTP API with OIDC authentication
- WebSocket real-time collaboration
- Multi-user roles and permissions
- Consensus governance for changes
- Complete audit trail with BFO provenance
- CLI commands from all loaded concepts

**Total composition time: 5 minutes. Zero integration code. All governed.**

---

## Building Your Own Concepts

```bash
# Create from template
ck concept create MyDomain.DataPipeline --template rust:cold --version v0.1

# Concept scaffold created at concepts/MyDomain.DataPipeline/
# Edit ontology.yaml to declare contracts, interfaces, governance
# Implement tool/rs/src/main.rs with your logic
# Test locally
ck concept start MyDomain.DataPipeline

# Export to cache for reuse
ck concept export MyDomain.DataPipeline --version v0.1

# Share with others
# concepts/MyDomain.DataPipeline_v0.1_aarch64-darwin_rs.tar.gz created
```

Your concept:
- Gets dynamic CLI commands if you declare `interfaces: [cli]`
- Participates in consensus if you declare governance boundaries
- Produces BFO-aligned evidence automatically
- Can be loaded into any ConceptKernel project
- Works with any runtime (Node.js or Rust)

---

## Documentation

- **[Protocol Specification](docs/CONCEPT-KERNEL.v1.3.14.md)** - High-level protocol and philosophy
- **[Low-Level Design](docs/CK_LLD.v1.3.14.md)** - Implementation details and architecture
- **[BFO Ontology](concepts/.ontology/README.md)** - Formal ontology mappings and coverage
- **[Development Guide](claude.md)** - Project context for contributors

---

## Installation

### Quick Install (Recommended)

**macOS / Linux:**
```bash
curl -sSL https://raw.githubusercontent.com/ConceptKernel/ck-core-rs/main/install.sh | sh
```

This will automatically detect your platform and install the latest version of `ckr`.

### Docker

```bash
# Pull the image
docker pull conceptkernel/ck-core-rs:latest

# Run ckr
docker run --rm conceptkernel/ck-core-rs:latest --version
```

See [docs/DOCKER.md](docs/DOCKER.md) for complete Docker usage guide.

### From GitHub Releases

Download pre-built binaries from [Releases](https://github.com/ConceptKernel/ck-core-rs/releases):

```bash
# Example for Linux x86_64
curl -L https://github.com/ConceptKernel/ck-core-rs/releases/download/v1.3.17/ckr-v1.3.17-x86_64-linux -o ckr
chmod +x ckr
sudo mv ckr /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/ConceptKernel/ck-core-rs.git
cd ck-core-rs
cargo build --release

# Binary at: target/release/ckr
# Add to PATH or symlink to /usr/local/bin/
```

**Requirements:**
- Rust 1.70+ (for building from source)
- No runtime dependencies (single binary deployment)

---

## Project Structure

```
ckp.v1.3.16.rust/
├── core-rs/              # Rust runtime implementation
│   ├── src/bin/          # ckr (CLI) + ckr-governor (watcher)
│   ├── src/kernel/       # Lifecycle, PID tracking, multi-project
│   ├── src/edge/         # Edge management, RBAC enforcement
│   ├── src/ontology/     # Parser, validator, CLI discovery
│   ├── src/cache/        # Package management
│   └── tests/            # 19 integration test suites
├── concepts/             # Loaded concept kernels
│   ├── .ontology/        # BFO formal ontology (RDF/OWL/TTL)
│   ├── System.*/         # System concepts (10 kernels)
│   └── UI.*/             # UI/demo concepts (6 kernels)
└── docs/                 # Protocol specs and design docs
```

---

## Development Status

**v1.3.16 - Active Development**

**Completed:**
- Core CLI with dynamic command discovery
- Multi-project isolation (port slots, registry, process detection)
- Package management (cache, load, export, import)
- PID:START_TIME validation (prevents PID reuse)
- 16 kernel implementations (System + UI)
- 19 integration test suites (Rust + shell)
- BFO ontology alignment (65% coverage)
- Ontology-driven CLI extension

**In Progress:**
- Governor queue watching (inotify/FSEvents)
- Full event emission with edge routing
- Process URN tracking (BFO Phase 1)

**Planned:**
- Three Mediators integration (EdgeKernel ↔ ConsensusKernel ↔ WssHubKernel)
- Automated multi-platform builds
- Remote registry protocol (concept-as-package-server)

---

## Contributing

ConceptKernel is a protocol-first system. Contributions must maintain:
- Protocol compatibility with Node.js runtime
- Filesystem conventions (URN → path resolution)
- PID:START_TIME format for process tracking
- BFO alignment for new entities/processes

Before contributing:
1. Read [claude.md](claude.md) for architecture context
2. Run test suite: `core-rs/tests/run-all-tests.sh`
3. Review BFO ontology: `concepts/.ontology/`
4. Update documentation alongside code

---

## License

MIT License - See [LICENSE](LICENSE)

---

## The Vision

**Background processes have a voice.**
**Critical decisions require consensus.**
**Every action produces proof.**
**Data stays isolated from software.**
**The filesystem is the protocol.**

ConceptKernel isn't middleware. It's a declaration of independence for autonomous processes to collaborate, govern, and prove their work—without coordination servers, external databases, or lock-in.

One command. Infinite compositions.

```bash
ck
```
