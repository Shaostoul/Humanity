USE NS project_universe;
USE DB science;

DEFINE TABLE meteorology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE meteorology TYPE string;
DEFINE FIELD description     ON TABLE meteorology TYPE string;
DEFINE FIELD type            ON TABLE meteorology TYPE string;
DEFINE FIELD phenomena       ON TABLE meteorology TYPE array;
DEFINE FIELD measurement     ON TABLE meteorology TYPE array;
DEFINE FIELD effects         ON TABLE meteorology TYPE array;
DEFINE FIELD related_fields  ON TABLE meteorology TYPE array;

DEFINE INDEX unique_meteorological_phenomenon ON TABLE meteorology COLUMNS name UNIQUE;

-- Example meteorological phenomena
CREATE meteorology:thunderstorm SET
  name = "Thunderstorm",
  description = "A storm characterized by the presence of lightning and its acoustic effect on the Earth's atmosphere, known as thunder",
  type = "Weather Phenomenon",
  phenomena = ["Lightning", "Thunder", "Heavy Rain"],
  measurement = ["Rain Gauge", "Lightning Detector"],
  effects = ["Flooding", "Power Outages"],
  related_fields = ["science:climatology", "science:atmospheric_science"];

CREATE meteorology:tornado SET
  name = "Tornado",
  description = "A rapidly rotating column of air that is in contact with both the surface of the Earth and a cumulonimbus cloud",
  type = "Weather Phenomenon",
  phenomena = ["Rotating Winds", "Funnel Cloud"],
  measurement = ["Doppler Radar", "Anemometer"],
  effects = ["Destruction", "Debris"],
  related_fields = ["science:climatology", "science:atmospheric_science"];

CREATE meteorology:hurricane SET
  name = "Hurricane",
  description = "A large rotating storm with high speed winds that forms over warm waters in tropical areas",
  type = "Weather Phenomenon",
  phenomena = ["High Winds", "Storm Surge", "Heavy Rain"],
  measurement = ["Satellite Imagery", "Barometer"],
  effects = ["Coastal Flooding", "Wind Damage"],
  related_fields = ["science:climatology", "science:atmospheric_science"];

CREATE meteorology:blizzard SET
  name = "Blizzard",
  description = "A severe snowstorm characterized by strong sustained winds and low visibility",
  type = "Weather Phenomenon",
  phenomena = ["Snow", "Strong Winds"],
  measurement = ["Anemometer", "Thermometer"],
  effects = ["Travel Disruptions", "Frostbite"],
  related_fields = ["science:climatology", "science:atmospheric_science"];

CREATE meteorology:heat_wave SET
  name = "Heat Wave",
  description = "A prolonged period of excessively hot weather, which may be accompanied by high humidity",
  type = "Weather Phenomenon",
  phenomena = ["High Temperatures", "Drought"],
  measurement = ["Thermometer", "Hygrometer"],
  effects = ["Heatstroke", "Water Shortages"],
  related_fields = ["science:climatology", "science:atmospheric_science"]; 