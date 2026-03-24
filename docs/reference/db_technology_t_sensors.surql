USE NS project_universe;
USE DB technology;
  
DEFINE TABLE sensors SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE sensors TYPE string;
DEFINE FIELD description     ON TABLE sensors TYPE string;
DEFINE FIELD type           ON TABLE sensors TYPE string;
DEFINE FIELD principle      ON TABLE sensors TYPE string;
DEFINE FIELD components     ON TABLE sensors TYPE array;
DEFINE FIELD requirements   ON TABLE sensors TYPE array;
DEFINE FIELD performance    ON TABLE sensors TYPE object;
DEFINE FIELD limitations    ON TABLE sensors TYPE array;
DEFINE FIELD applications   ON TABLE sensors TYPE array;
DEFINE FIELD related_systems ON TABLE sensors TYPE array;
  
DEFINE INDEX unique_sensor_system ON TABLE sensors COLUMNS name UNIQUE;
DEFINE INDEX sensor_type ON TABLE sensors COLUMNS type;
  
-- Optical Sensors
CREATE sensors:optical_sensor SET
  name = "Optical Sensor Array",
  description = "Advanced optical sensing system for imaging and spectroscopy",
  type = "Optical",
  principle = "Electromagnetic radiation detection",
  components = ["CCD/CMOS detector", "Optical elements", "Filter wheel", "Cooling system"],
  requirements = ["Power supply", "Temperature control", "Calibration", "Data processing"],
  performance = {
    resolution: "1-100 megapixels",
    wavelength_range: "200-2000 nm",
    sensitivity: "0.1-100 photons/pixel",
    frame_rate: "1-1000 fps"
  },
  limitations = ["Radiation damage", "Temperature sensitivity", "Calibration drift"],
  applications = ["Astronomical imaging", "Remote sensing", "Spectroscopy"],
  related_systems = ["power:power_distribution", "communication:data_processor"];
  
-- Radiation Detector
CREATE sensors:radiation_detector SET
  name = "Radiation Detection System",
  description = "Comprehensive radiation monitoring and detection system",
  type = "Radiation",
  principle = "Ionizing radiation detection",
  components = ["Scintillator", "Photomultiplier", "Electronics", "Shielding"],
  requirements = ["Power supply", "Calibration", "Shielding", "Data processing"],
  performance = {
    energy_range: "10 keV - 10 MeV",
    sensitivity: "0.1-1000 counts/s",
    resolution: "5-20% FWHM",
    response_time: "<1 ms"
  },
  limitations = ["Dead time", "Background radiation", "Energy resolution"],
  applications = ["Radiation monitoring", "Particle detection", "Environmental survey"],
  related_systems = ["power:power_distribution", "communication:data_processor"];
  
-- Environmental Sensor
CREATE sensors:environmental_sensor SET
  name = "Environmental Monitoring System",
  description = "Multi-parameter environmental sensing system",
  type = "Environmental",
  principle = "Physical and chemical parameter measurement",
  components = ["Temperature sensor", "Pressure sensor", "Gas analyzer", "Humidity sensor"],
  requirements = ["Power supply", "Calibration", "Maintenance", "Data processing"],
  performance = {
    temperature_range: "-50 to 150°C",
    pressure_range: "0.1-1000 kPa",
    gas_sensitivity: "1-1000 ppm",
    response_time: "<1 s"
  },
  limitations = ["Sensor drift", "Cross-sensitivity", "Calibration requirements"],
  applications = ["Life support monitoring", "Environmental control", "Safety systems"],
  related_systems = ["power:power_distribution", "life_support:air_recycling"];
  
-- Navigation Sensor
CREATE sensors:navigation_sensor SET
  name = "Navigation and Guidance System",
  description = "Advanced navigation and attitude determination system",
  type = "Navigation",
  principle = "Inertial and celestial navigation",
  components = ["IMU", "Star tracker", "GPS receiver", "Data processor"],
  requirements = ["Power supply", "Calibration", "Star catalog", "Data processing"],
  performance = {
    position_accuracy: "1-100 m",
    attitude_accuracy: "0.01-1 arcsec",
    update_rate: "1-100 Hz",
    reliability: "99.99%"
  },
  limitations = ["GPS availability", "Star visibility", "IMU drift"],
  applications = ["Spacecraft navigation", "Attitude control", "Trajectory planning"],
  related_systems = ["power:power_distribution", "communication:data_processor"]; 