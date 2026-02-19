USE NS project_universe;
USE DB technology;

DEFINE TABLE computing SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE computing TYPE string;
DEFINE FIELD description     ON TABLE computing TYPE string;
DEFINE FIELD type            ON TABLE computing TYPE string;
DEFINE FIELD architecture    ON TABLE computing TYPE string;
DEFINE FIELD components      ON TABLE computing TYPE array;
DEFINE FIELD performance     ON TABLE computing TYPE object;
DEFINE FIELD limitations     ON TABLE computing TYPE array;
DEFINE FIELD applications    ON TABLE computing TYPE array;
DEFINE FIELD related_systems ON TABLE computing TYPE array;

DEFINE INDEX unique_computing_name ON TABLE computing COLUMNS name UNIQUE;

-- Example computing systems
CREATE computing:quantum_computer SET
  name = "Quantum Computer",
  description = "Advanced computing system using quantum bits for processing",
  type = "Quantum",
  architecture = "Qubit-based",
  components = ["Qubits", "Quantum Gates", "Quantum Processor"],
  performance = {qubit_count: "50-1000", speedup: "Exponential over classical"},
  limitations = ["Decoherence", "Error Rates"],
  applications = ["Cryptography", "Complex Simulations"],
  related_systems = ["technology:AI", "technology:networking"];

CREATE computing:supercomputer SET
  name = "Supercomputer",
  description = "High-performance computing system for complex calculations",
  type = "Classical",
  architecture = "Parallel Processing",
  components = ["CPUs", "GPUs", "High-speed Interconnects"],
  performance = {flops: "Petaflops to Exaflops", memory: "Terabytes"},
  limitations = ["High Power Consumption", "Cooling Requirements"],
  applications = ["Weather Forecasting", "Scientific Research"],
  related_systems = ["technology:cooling", "technology:storage"];

CREATE computing:cloud_computing SET
  name = "Cloud Computing",
  description = "Distributed computing system providing on-demand resources",
  type = "Distributed",
  architecture = "Virtualized Infrastructure",
  components = ["Servers", "Virtual Machines", "Network Infrastructure"],
  performance = {scalability: "Elastic", availability: "99.9% uptime"},
  limitations = ["Latency", "Security Concerns"],
  applications = ["Web Hosting", "Data Analytics"],
  related_systems = ["technology:networking", "technology:security"];

CREATE computing:edge_computing SET
  name = "Edge Computing",
  description = "Decentralized computing system processing data at the edge of the network",
  type = "Decentralized",
  architecture = "Edge Nodes",
  components = ["Edge Devices", "Local Servers", "IoT Sensors"],
  performance = {latency: "Low", bandwidth_usage: "Reduced"},
  limitations = ["Limited Processing Power", "Data Management"],
  applications = ["IoT Applications", "Real-time Processing"],
  related_systems = ["technology:IoT", "technology:networking"];
