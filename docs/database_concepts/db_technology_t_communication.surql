USE NS project_universe;
USE DB technology;
  
DEFINE TABLE communication SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE communication TYPE string;
DEFINE FIELD description     ON TABLE communication TYPE string;
DEFINE FIELD type           ON TABLE communication TYPE string;
DEFINE FIELD principle      ON TABLE communication TYPE string;
DEFINE FIELD components     ON TABLE communication TYPE array;
DEFINE FIELD requirements   ON TABLE communication TYPE array;
DEFINE FIELD performance    ON TABLE communication TYPE object;
DEFINE FIELD limitations    ON TABLE communication TYPE array;
DEFINE FIELD applications   ON TABLE communication TYPE array;
DEFINE FIELD related_systems ON TABLE communication TYPE array;
  
DEFINE INDEX unique_communication_system ON TABLE communication COLUMNS name UNIQUE;
DEFINE INDEX communication_type ON TABLE communication COLUMNS type;
  
-- Radio Communication
CREATE communication:radio_network SET
  name = "Radio Communication Network",
  description = "Traditional radio-based communication system",
  type = "Radio",
  principle = "Electromagnetic wave transmission",
  components = ["Antenna array", "Transceiver", "Modulator", "Signal processor"],
  requirements = ["Power supply", "Frequency allocation", "Antenna pointing", "Signal processing"],
  performance = {
    frequency_range: "1-100 GHz",
    bandwidth: "1-1000 MHz",
    data_rate: "1-1000 Mbps",
    range: "100-1000000 km"
  },
  limitations = ["Speed of light delay", "Signal attenuation", "Interference"],
  applications = ["Spacecraft communication", "Space station links", "Surface base communication"],
  related_systems = ["power:power_distribution", "sensors:signal_processor"];
  
-- Laser Communication
CREATE communication:laser_link SET
  name = "Laser Communication Link",
  description = "High-bandwidth optical communication system",
  type = "Optical",
  principle = "Laser beam transmission",
  components = ["Laser transmitter", "Optical receiver", "Pointing system", "Signal processor"],
  requirements = ["Power supply", "Precise pointing", "Atmospheric compensation", "Signal processing"],
  performance = {
    wavelength: "800-1600 nm",
    bandwidth: "1-100 GHz",
    data_rate: "1-100 Gbps",
    range: "1000-1000000 km"
  },
  limitations = ["Line of sight required", "Atmospheric effects", "Pointing accuracy"],
  applications = ["Deep space communication", "High-bandwidth links", "Secure communication"],
  related_systems = ["power:power_distribution", "sensors:optical_sensor"];
  
-- Quantum Communication
CREATE communication:quantum_network SET
  name = "Quantum Communication Network",
  description = "Advanced quantum-based secure communication system",
  type = "Quantum",
  principle = "Quantum entanglement and key distribution",
  components = ["Quantum source", "Entanglement generator", "Quantum receiver", "Key processor"],
  requirements = ["Power supply", "Temperature control", "Isolation", "Quantum processing"],
  performance = {
    key_rate: "1-1000 kbps",
    security: "Unconditionally secure",
    range: "100-1000 km",
    reliability: "99.99%"
  },
  limitations = ["Limited range", "Complex technology", "High power requirements"],
  applications = ["Secure communication", "Quantum key distribution", "Military operations"],
  related_systems = ["power:power_distribution", "sensors:quantum_detector"];
  
-- Data Processing
CREATE communication:data_processor SET
  name = "Data Processing System",
  description = "System for processing and managing communication data",
  type = "Processing",
  principle = "Digital signal processing and data management",
  components = ["Processor array", "Memory system", "Data bus", "Error correction"],
  requirements = ["Power supply", "Cooling", "Memory", "Processing capacity"],
  performance = {
    processing_speed: "1-1000 TFLOPS",
    memory_capacity: "1-1000 TB",
    error_rate: "<10^-12",
    latency: "<1 ms"
  },
  limitations = ["Heat generation", "Power consumption", "Complex maintenance"],
  applications = ["Signal processing", "Data compression", "Error correction"],
  related_systems = ["power:power_distribution", "life_support:thermal_control"]; 