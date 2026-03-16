USE NS project_universe;
USE DB engineering;

DEFINE TABLE structural_engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE structural_engineering TYPE string;
DEFINE FIELD description     ON TABLE structural_engineering TYPE string;
DEFINE FIELD type            ON TABLE structural_engineering TYPE string;
DEFINE FIELD materials       ON TABLE structural_engineering TYPE array;
DEFINE FIELD applications    ON TABLE structural_engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE structural_engineering TYPE array;

DEFINE INDEX unique_structural_engineering_name ON TABLE structural_engineering COLUMNS name UNIQUE;

-- Example structural engineering concepts
CREATE structural_engineering:bridge_design SET
  name = "Bridge Design",
  description = "The process of designing bridges to ensure they are safe, functional, and aesthetically pleasing",
  type = "Field",
  materials = ["Steel", "Concrete"],
  applications = ["Transportation Infrastructure", "Civil Engineering Projects"],
  related_fields = ["engineering:civil_engineering", "engineering:architecture"];

CREATE structural_engineering:building_construction SET
  name = "Building Construction",
  description = "The process of adding structure to real property",
  type = "Field",
  materials = ["Wood", "Glass"],
  applications = ["Residential Buildings", "Commercial Buildings"],
  related_fields = ["engineering:construction_management", "engineering:urban_planning"];

CREATE structural_engineering:earthquake_engineering SET
  name = "Earthquake Engineering",
  description = "The study of the behavior of structures subject to seismic loading",
  type = "Field",
  materials = ["Reinforced Concrete", "Base Isolators"],
  applications = ["Seismic Retrofitting", "Disaster Mitigation"],
  related_fields = ["engineering:geotechnical_engineering", "engineering:disaster_management"];
