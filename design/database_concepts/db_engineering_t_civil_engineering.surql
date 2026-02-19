USE NS project_universe;
USE DB engineering;

DEFINE TABLE civil_engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE civil_engineering TYPE string;
DEFINE FIELD description     ON TABLE civil_engineering TYPE string;
DEFINE FIELD type            ON TABLE civil_engineering TYPE string;
DEFINE FIELD components      ON TABLE civil_engineering TYPE array;
DEFINE FIELD applications    ON TABLE civil_engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE civil_engineering TYPE array;

DEFINE INDEX unique_civil_engineering_name ON TABLE civil_engineering COLUMNS name UNIQUE;

-- Example civil engineering concepts
CREATE civil_engineering:structural_engineering SET
  name = "Structural Engineering",
  description = "The branch of civil engineering that deals with the design and analysis of structures that support or resist loads",
  type = "Field",
  components = ["Beams", "Columns"],
  applications = ["Building Design", "Bridge Construction"],
  related_fields = ["engineering:architecture", "engineering:materials"];

CREATE civil_engineering:geotechnical_engineering SET
  name = "Geotechnical Engineering",
  description = "The branch of civil engineering concerned with the engineering behavior of earth materials",
  type = "Field",
  components = ["Soil Analysis", "Foundation Design"],
  applications = ["Earthquake Engineering", "Slope Stability"],
  related_fields = ["engineering:environmental", "engineering:geology"];

CREATE civil_engineering:transportation_engineering SET
  name = "Transportation Engineering",
  description = "The application of technology and scientific principles to the planning, functional design, operation, and management of facilities for any mode of transportation",
  type = "Field",
  components = ["Traffic Analysis", "Highway Design"],
  applications = ["Urban Planning", "Public Transit Systems"],
  related_fields = ["engineering:urban_planning", "engineering:logistics"]; 