# Changelog

All notable changes to ConceptKernel documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/)
Versioning follows [Semantic Versioning](https://semver.org/)

---

## [1.3.20-alpha.2] - 2026-01-08

**Alpha Release:** Test Infrastructure + Browser NATS + Ontology Validation

### Added

#### Test Infrastructure
- **10 Comprehensive Integration Test Suites** (800+ lines)
  - `diskless_workflow_integration_tests.rs` - Filesystem-independent workflow testing
  - `edge_router_diskless_discovery_tests.rs` - Edge discovery validation
  - `jena_storage_driver_tests.rs` - RDF storage CRUD operations
  - `jena_transaction_tests.rs` - ACID transaction verification
  - `kernel_startup_occurrent_tests.rs` - BFO Process tracking tests
  - `ontology_validation_tests.rs` - Complete ontology validation
  - `use_case_verification_tests.rs` - End-to-end scenarios
  - `workflow_lifecycle_validation_test.rs` - Full workflow lifecycle
  - `workflow_tx_jena_validation.rs` - Transaction integrity tests

#### Test.Nats Reference Kernel
- **Browser Integration Demo** (`concepts/Test.Nats/`)
  - Zero-configuration NATS WebSocket from browsers
  - 10 messages @ 2000ms intervals verified
  - Integrated with @conceptkernel/ck-client-js v1.3.22
  - Complete message flow documentation

#### JavaScript Client Library v1.3.22
- **Browser-native NATS support** - Automatic CDN loading
- **New publishToSubject() API** - Direct NATS messaging
- **Smart environment detection** - Browser vs Node.js auto-routing
- See `ck-client-js/RELEASE_NOTES_v1.3.22.md` for details

### Fixed

#### Jena Fuseki SPARQL Endpoints
- **Non-standard endpoint configuration** resolved
  - Use base endpoint (`/dataset`) for all operations
  - Content-Type header determines operation type
  - Fixed 405 Method Not Allowed errors
  - Updated: `core-rs/src/drivers/storage/jena.rs`

#### SPARQL FILTER Queries
- **String vs URI comparison** fixed
  - `FILTER(?urn = "string")` instead of `FILTER(?urn = <uri>)`
  - Process occurrent queries now work correctly
  - BFO compliance validation enabled

### Documentation

- **New Docs:**
  - `/tmp/test-nats-payload-flow.md` - Complete message flow
  - `/tmp/test-nats-implementation-verified.md` - Integration proof
  - `/tmp/test-nats-diagnosis-v2.md` - Browser debugging guide
  - `core-rs/tests/integration/README.md` - Test suite guide

- **Updated:**
  - `CLAUDE.md` - Jena endpoint configuration warnings
  - `CHANGELOG_OCCURRENT_TRACKING.md` - Detailed occurrent tracking

### Known Issues

- Playwright tests require manual install: `npm install -D @playwright/test`
- Browser tests need WebSocket port-forward: `kubectl port-forward -n nats svc/nats 8080:8080`

---

## [1.3.20-alpha.1] - 2025-12-31

**Initial Alpha Release:** Driver Abstraction Layer + Semantic Validation

(See v1.3.20 entry below for complete feature list)

---

## [1.3.20] - 2025-12-31

**Major Release:** Driver Abstraction Layer + Semantic Validation + Multi-Mode Execution

### Added

#### Core Architecture
- **Driver Abstraction Layer** - Pluggable storage and transport backends
  - `StorageDriver` trait with 13 async methods for data persistence
  - `TransportDriver` trait with 8 async methods for message passing
  - `DriverFactory` for configuration-based driver instantiation
  - Complete async/await API for non-blocking operations

#### Storage Drivers
- **JenaStorage** (1,605 lines) - Apache Jena Fuseki RDF triple store backend
  - SPARQL query support for semantic reasoning
  - RDF/Turtle ontology parsing and validation
  - BFO compliance checking via SPARQL ASK queries
  - Edge authorization validation
  - Instance provenance tracking
  - Persistent TDB2 storage
- **LocalStorage** (264 lines) - Async wrapper around FileSystemDriver
  - Backward compatibility with v1.3.19 filesystem-based deployments
  - `tokio::spawn_blocking` for async filesystem operations

#### Transport Drivers
- **NatsTransport** (814 lines) - NATS JetStream message queue backend
  - JetStream consumer management for durable queues
  - Subject-based routing: `{kernel}/jobs`, `{kernel}/results`, `{target}/edges/{predicate}/{source}`
  - MsgPack binary encoding (81% size reduction vs JSON)
  - At-least-once delivery guarantees
  - Message acknowledgment handling
  - WebSocket gateway support for browser clients
- **LocalTransport** (437 lines) - Filesystem + notify crate wrapper
  - File watcher integration for event-driven job detection
  - Backward compatibility with v1.3.19

#### Tool Execution Modes
- **Four execution strategies** - Governor dynamically selects optimal mode
  - **Cold (Kubernetes Job)**: Ephemeral containers for batch processing (5-30s startup)
  - **Hot (Service)**: Always-running pods for real-time operations (<10ms latency)
  - **API Wrapper**: HTTP/gRPC external service integration (100-500ms)
  - **Preinstalled Binary**: Tools bundled in governor container (<1ms startup)
- Auto-detection with fallback chain based on kernel configuration
- Per-tool resource limits and timeout configuration

#### Semantic Validation (Jena-based)
- **Protocol Ontologies as Validator** - Jena Fuseki enforces ALL semantic integrity rules
  - Edge authorization via SPARQL: `ckp:isAuthorized true` required
  - Instance provenance: `ckp:createdByKernel` mandatory
  - Communication constraint: Kernels ONLY communicate via authorized edges
  - Predicate validity: Only approved predicates (PRODUCES, REQUIRES, NOTIFIES, etc.)
- SWRL inference rules for automatic reasoning
  - Workflow chain inference (transitive dependencies)
  - Provenance propagation via property chains
- Validation methods:
  - `validate_kernel_ontology()` - Check BFO compliance
  - `validate_edge_authorization()` - SPARQL ASK query for edge admission
  - `find_unauthorized_edges()` - Integrity audit
  - `save_edge_metadata()` - RDF edge persistence

#### Event System (36-Event Taxonomy)
- Phase-based lifecycle events for kernels
  - Boot phase: `kernel.boot.started`, `kernel.boot.completed`
  - Setup phase: `kernel.setup.started`, `kernel.ontology.loaded`
  - Activation: `kernel.activated`
  - Operation: `kernel.job.received`, `kernel.tool.executed`, `kernel.result.published`
  - Maintenance: `kernel.heartbeat`, `kernel.metrics.reported`
  - Shutdown: `kernel.shutdown.started`, `kernel.shutdown.completed`
- Edge lifecycle events: `edge.created`, `edge.authorized`, `edge.notification.sent`
- Transaction events: `tx.started`, `tx.committed`, `tx.failed`
- Consensus events: `consensus.proposed`, `consensus.voted`, `consensus.reached`

#### Async Edge Routing
- **EdgeRouterDaemonAsync** - Transport-based async edge routing (replaced synchronous notify-based)
  - Subscribe to result streams: `{kernel}/results`
  - Parse `notification_contract` from kernel ontology
  - Validate edge authorization with Jena before routing
  - Publish to edge queues: `{target}/edges/{predicate}/{source}`
  - Process URN tracking for provenance
  - Support for both LocalTransport and NatsTransport

#### Configuration
- **Extended .ckproject** with `backends` section
  ```yaml
  backends:
    storage:
      type: jena | local
      fuseki_url: http://jena:3030/conceptkernel
    transport:
      type: nats | local
      url: nats://nats:4222
      websocket: ws://nats:8080
  ```
- Environment variable overrides: `CKP_STORAGE_TYPE`, `CKP_TRANSPORT_TYPE`
- Hybrid migration support: NATS events + filesystem fallback

#### Deployment
- **Stateless Kubernetes containers** - One container per ConceptKernel
  - Same image, different env vars (`KERNEL_NAME`, `NATS_URL`, `JENA_URL`)
  - No persistent volumes required
  - Horizontal Pod Autoscaling (HPA) based on queue depth
  - Operator-managed configuration
- **Operator CRD Support** (v1alpha2 planned)
  - ConceptKernel CRD with execution mode config
  - NATS subject provisioning
  - Jena dataset initialization
  - SHACL validation integration
- Helm charts for ck-operator, NATS, Jena Fuseki

#### Protocol Enhancements
- **MsgPack Binary Encoding** - Optional alternative to JSON
  - 81% size reduction (15 bytes vs 80 bytes for typical messages)
  - 10,000 msg/sec throughput, <10ms latency
  - Rust: `rmp-serde`, JavaScript: `msgpackr`
  - Compact encoding best practices documented
- **URN Schema Evolution** - Extended URN format for drivers
  - Process URNs: `ckp://Process#{KernelAction}-{txId}`
  - Edge URNs: `ckp://Edge.{predicate}.{source}-to-{target}:v{version}`
  - Storage artifact URNs with driver prefix

#### Documentation (Complete v1.3.20 Spec)
- **Core Specs (CORE.*.md)**: 8 documents
  - CORE.DRIVERS.md - Driver abstraction architecture
  - CORE.GOVERNOR.md - Governor with driver integration
  - CORE.STATELESS-DESIGN.md - Stateless container patterns
  - CORE.EVENTS.md - 36-event taxonomy
  - CORE.CONCEPTKERNEL.md, CORE.EDGES.md, CORE.PROCESSES.md, CORE.ONTOLOGY.md
- **Feature Specs (FEATURE.*.md)**: 11 documents
  - FEATURE.EXECUTION-MODES.md - Tool execution strategies
  - FEATURE.STORAGE-JENA.md, FEATURE.STORAGE-LOCAL.md, FEATURE.STORAGE-AGE.md, FEATURE.STORAGE-SEAWEEDFS.md
  - FEATURE.TRANSPORT-NATS.md, FEATURE.TRANSPORT-LOCAL.md, FEATURE.TRANSPORT-WEBSOCKET.md
  - FEATURE.EDGE-ROUTING.md, FEATURE.PACKAGING.md, FEATURE.WORKFLOWS.md
- **Protocol Specs (SPEC.*.md)**: 7 documents
  - SPEC.PROTOCOL.md - ConceptKernel Protocol v1.3.20
  - SPEC.MSGPACK.md - Binary encoding specification
  - SPEC.KUBERNETES-CRD.md - Operator CRD design
  - SPEC.EVENT-PHASES.md - Event lifecycle taxonomy
  - SPEC.SPARQL.md, SPEC.KERNEL-API.md, SPEC.EDGE-PREDICATES.md
- **Implementation Guides**: 24 chapters (00-23)
  - Complete phased implementation roadmap
  - Driver trait definitions and implementations
  - Governor refactor guide
  - Deployment patterns and examples

### Changed

#### Governor Architecture
- Refactored `ConceptKernelGovernor` to use driver abstraction
  - Now accepts `Arc<dyn StorageDriver>` (was hardcoded `FileSystemDriver`)
  - Added optional `Arc<dyn TransportDriver>` for async messaging
  - Optional `JenaStorage` integration for semantic validation
  - Event loop migrated from `notify` crate to `TransportDriver::subscribe_inbox()`
  - Tool spawning delegated to `ToolExecutor` trait (execution mode strategy)
- Backward-compatible constructors: `new()` uses LocalStorage + LocalTransport
- New constructor: `new_with_drivers()` for v1.3.20 deployments

#### Edge Router
- Migrated from synchronous filesystem watching to async transport streams
  - Uses `TransportDriver::subscribe_results()` instead of notify crate
  - Publishes to edge queues via `TransportDriver::publish_edge_notification()`
- Added Jena edge authorization checks before routing
  - SPARQL validation: `validate_edge_authorization(source, target, predicate)`
  - Blocks unauthorized edges at routing time (enforcement point)
- Process URN tracking for edge routing provenance

#### Configuration System
- Extended `.ckproject` with `backends` section (storage + transport)
- Added `DriverFactory::from_config()` for declarative driver instantiation
- Environment variable override support (`CKP_STORAGE_TYPE`, `CKP_TRANSPORT_TYPE`)
- Backward compatible: Missing `backends` section defaults to filesystem + local transport

#### FileSystemDriver
- All 8 existing methods converted to async (uses `tokio::fs`)
- Added 5 new methods:
  - `load_ontology()` - Read RDF or YAML ontology
  - `save_ontology()` - Write RDF ontology
  - `load_tool_definition()` - Parse tool config from ontology
  - `save_result()` - Write tool response to results/
  - `load_result()` - Read tool response from results/

#### Ontology Handling
- RDF ontology support via Jena (primary) or filesystem (fallback)
- Ontology validation moved to JenaStorage (SPARQL-based, not Rust code)
- Edge predicate validation uses loaded protocol ontologies
- SWRL rule execution for automatic inference

#### Binary Naming
- Binary renamed: `ckr` → `ckp` (ConceptKernel Protocol)
  - Update scripts: `ckp start System.Wss` (was `ckr start System.Wss`)

### Deprecated

- **Synchronous storage methods** - Use async `StorageDriver` trait methods
- **Direct `notify` crate usage in Governor** - Use `TransportDriver::subscribe_inbox()`
- **Hardcoded filesystem paths in Governor** - Use `StorageDriver::resolve_urn()`
- **YAML-only ontologies** - Prefer RDF/Turtle (Jena can import YAML if needed)

### Removed

- None (v1.3.20 maintains full backward compatibility)

### Fixed

- **Event loop blocking** - Replaced synchronous notify with async streams
- **Edge routing latency** - NATS event-driven routing (was 1s filesystem polling, now <10ms)
- **Governor restart recovery** - JetStream durable queues preserve jobs across restarts
- **Ontology validation duplication** - Single source of truth in Jena (was duplicated in Rust)
- **Race conditions in job processing** - Async mutex guards for tool execution state

### Security

- **Edge authorization enforcement** - Jena SPARQL validation before routing
- **Instance provenance tracking** - Every instance MUST have `ckp:createdByKernel`
- **Unauthorized edge detection** - Periodic integrity audits via `find_unauthorized_edges()`
- **SHACL validation** (Jena) - Reject malformed kernel ontologies at admission

### Performance

- **Edge routing latency**: 1000ms → 10ms (NATS event-driven vs filesystem polling)
- **Message size**: -81% (MsgPack binary encoding vs JSON)
- **Throughput**: 10,000 msg/sec (NATS JetStream)
- **Query performance**: <100ms SPARQL queries (Jena with TDB2 indexes)
- **Container footprint**: ~20-30MB (distroless + stripped binary)

### Implementation Statistics

- **New code**: ~5,010 lines (driver traits + implementations)
  - Driver traits: 1,073 lines
  - JenaStorage: 1,605 lines
  - NatsTransport: 814 lines
  - LocalStorage: 264 lines
  - LocalTransport: 437 lines
  - DriverFactory: 817 lines
- **Documentation**: 50+ new specification files
- **Test coverage**: Integration tests for all driver combinations

---

## [1.3.19] - 2025-12-08

**Release Type:** Minor version with major feature additions

### Added

#### Ontology Auto-Generation System
- **File:** `core-rs/src/ontology/generator.rs` (364 lines)
- Automatic `ontology.ttl` generation for forked and created kernels
- Inherits roles and functions from source kernels
- Uses `kernel-entity-template.ttl` for consistency
- Ensures BFO compliance for all generated ontologies
- Eliminates manual ontology creation errors
- Maintains ontological consistency across kernel hierarchies

#### Self-Improvement API
- **File:** `core-rs/src/ontology/improvement.rs` (307 lines)
- Comprehensive validation and improvement system:
  - `ValidationIssue` - Detects ontology compliance issues
  - `IssueSeverity` - Critical, High, Medium, Low classifications
  - `IssueType` - Missing predicates, invalid types, incomplete metadata
  - `ImprovementRecommendation` - Structured improvement proposals
- Validates kernel ontologies against BFO specifications
- Generates improvement recommendations
- Queries validation issues by severity
- Submits recommendations to consensus mechanisms
- Triggers improvement processes via kernel actions

#### Workflow System (CKDL Support)
- **Directory:** `core-rs/src/workflow/` (3 files, 1,241 lines total)
- **CKDL Parser** (`ckdl_parser.rs` - 496 lines) - Parses Concept Kernel Definition Language
- **Workflow Module** (`mod.rs` - 350 lines) - Workflow execution and management
- **Validator** (`validator.rs` - 395 lines) - Workflow validation and compliance checks
- Define complex kernel workflows in CKDL
- Parse and validate workflow definitions
- Execute multi-step processes with dependencies
- Support for conditional execution and error handling

#### URN CKDL Parser
- **File:** `core-rs/src/urn/ckdl_parser.rs` (326 lines)
- Specialized CKDL parser for URN system
- Parses URN definitions from CKDL syntax
- Validates URN structure and components
- Supports Process URN format: `ckp://Process#KernelAction-txId`
- Integrates with workflow system for URN resolution

### Changed

#### Kernel Governor System
- **File:** `core-rs/src/kernel/governor.rs` (+255 lines)
- Enhanced kernel lifecycle management
- Improved state transition handling
- Advanced resource governance
- Better error recovery mechanisms
- Performance optimizations

#### Ontology Library System
- **File:** `core-rs/src/ontology/library.rs` (+408 lines)
- Expanded ontology management capabilities
- Enhanced role and function metadata handling
- Improved ontology inheritance mechanisms
- Better validation and compliance checking
- Support for ontology generation integration

#### Ontology Query System
- **File:** `core-rs/src/ontology/query.rs` (+359 lines)
- Advanced query capabilities
- SPARQL integration improvements
- Flexible filtering and sorting
- Better Oxigraph integration
- Support for complex ontology queries

#### Port Management System
- **File:** `core-rs/src/port/manager.rs` (+342 lines)
- Comprehensive port management
- Enhanced port allocation algorithms
- Better conflict resolution
- Improved port tracking and lifecycle
- Support for dynamic port ranges

#### Binary/CLI
- Renamed: `ckr` → `ckp` (`core-rs/src/bin/ckp.rs`, +214 lines)
- Enhanced command capabilities
- Better error messages and help text

#### Tracking Systems
- **ContinuantTracker** - Improved kernel entity tracking
- **ProcessTracker** - Enhanced process URN management (~48 line changes)
- Better integration with Oxigraph for RDF queries

#### Edge & Daemon Systems
- **EdgeRouter** - Improved edge queue handling (~8 line changes)
- **EdgeKernel** - Enhanced edge communication (~92 line changes)
- **RequestBuilder** - Better request construction (~6 line changes)

#### Drivers
- **Filesystem Driver** - Enhanced file operations (+91 lines)
- **Driver Module** - New capabilities (+4 lines)

#### Kernel Management
- **API** - Expanded public API surface (~38 line changes)
- **Builder** - Improved kernel construction
- **Manager** - Better kernel lifecycle management (~63 line changes)
- **Module** - Enhanced exports (~9 line changes)

#### URN System
- **URN Module** - Improved URN handling (~36 line changes)
- **Resolver** - Better URN resolution
- **Validator** - Enhanced URN validation
- **CKDL Parser** - New CKDL support (298 lines)

#### Storage & Project
- **Project Config** - Enhanced configuration handling (~23 line changes)
- **Storage Module** - Improved storage operations

#### Public API
- **File:** `core-rs/src/lib.rs` (~34 line changes)
- Exported new modules: `OntologyGenerator`, `ImprovementAPI`, `WorkflowSystem`
- Enhanced existing exports

### Infrastructure

#### Docker Support
- Updated `Dockerfile` with optimizations
- Updated `Dockerfile.prebuilt` for faster builds
- Better multi-stage build process

#### CI/CD Pipeline
- Updated `.github/workflows/release.yml`
- Automated build verification
- Container image generation
- Cross-platform builds

#### Installation
- Enhanced `install.sh` script
- Better platform detection
- Improved error handling

#### Dependencies
- Updated `Cargo.toml` with new dependencies
- Refreshed `Cargo.lock` with latest versions

### Testing

#### New Test Contracts
- `status_tool_path_contracts.rs` (304 lines) - Status and tool path validation

#### Updated Integration Tests (8 files)
- Kernel manager contracts
- Port allocation contracts
- Process tracker contracts
- Edge router integration tests
- Kernel integration tests
- Kernel lifecycle tests
- Portable CLI tests

### Statistics

- **Total additions:** +2,757 lines
- **Total deletions:** -524 lines
- **Net new functionality:** +2,233 lines
- **New files added:** 2,542 lines (6 files)
- **Modified tracked files:** 42 files
- **Total files in release:** 47 files

---

## [1.3.18] - 2024-12-04

### Added
- Enhanced consensus mechanisms
- Improved edge routing protocols
- Extended ontology processing
- Additional kernel lifecycle events

### Changed
- Refined governor state management
- Updated ontology library interfaces
- Improved process tracking accuracy

---

## [1.3.16] - 2024-12-02

### Added
- Initial ConceptKernel core library release
- FileSystemDriver for local storage
- Basic governor implementation
- Kernel lifecycle management
- Edge routing foundation
- URN resolution system
- Ontology library
- Port management
- Process tracking
- Basic CLI (`ckr`)

### Features
- BFO-compliant ontologies
- Kernel forking and versioning
- Edge-based communication
- Process provenance tracking
- SPARQL query integration (Oxigraph)

---

## Links

- **Repository**: https://github.com/ConceptKernel/ckp
- **Documentation**: https://conceptkernel.org/docs
- **Issue Tracker**: https://github.com/ConceptKernel/ckp/issues
- **Changelog**: https://github.com/ConceptKernel/ckp/blob/main/CHANGELOG.md

---

## Migration Guides

### Upgrading from v1.3.19 to v1.3.20

**No code changes required** for existing deployments. v1.3.20 is fully backward compatible.

**Optional: Adopt new drivers**

1. Add `backends` section to `.ckproject`:
   ```yaml
   backends:
     storage:
       type: jena
       fuseki_url: http://jena:3030/conceptkernel
     transport:
       type: nats
       url: nats://nats:4222
   ```

2. Deploy NATS and Jena:
   ```bash
   kubectl apply -f k8s/nats.yaml
   kubectl apply -f k8s/jena-fuseki.yaml
   ```

3. Load protocol ontologies:
   ```bash
   ./scripts/init-jena-ontologies.sh http://jena:3030 conceptkernel
   ```

4. Restart governors (automatically detect new drivers):
   ```bash
   ckp governor start System.Registry
   ```

**Update binary name in scripts**:
```bash
# Old (v1.3.19)
ckr start System.Wss

# New (v1.3.20)
ckp start System.Wss
```

### Upgrading from v1.3.18 to v1.3.19

See [RELEASE_NOTES_v1.3.19.md](RELEASE_NOTES_v1.3.19.md) for detailed migration guide.

**Key changes**:
- Binary renamed from `ckr` to `ckp`
- New ontology auto-generation system
- Workflow system with CKDL support
- Self-improvement API
