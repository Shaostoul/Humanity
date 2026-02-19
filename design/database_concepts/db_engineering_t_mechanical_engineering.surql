USE NS project_universe;
USE DB engineering;

DEFINE TABLE mechanical_engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE mechanical_engineering TYPE string;
DEFINE FIELD description     ON TABLE mechanical_engineering TYPE string;
DEFINE FIELD type            ON TABLE mechanical_engineering TYPE string;
DEFINE FIELD components      ON TABLE mechanical_engineering TYPE array;
DEFINE FIELD applications    ON TABLE mechanical_engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE mechanical_engineering TYPE array;

DEFINE INDEX unique_mechanical_engineering_name ON TABLE mechanical_engineering COLUMNS name UNIQUE;

-- Example mechanical engineering concepts
CREATE mechanical_engineering:thermodynamics SET
  name = "Thermodynamics",
  description = "The branch of physical science that deals with the relations between heat and other forms of energy",
  type = "Field",
  components = ["Heat Engines", "Refrigeration Systems"],
  applications = ["Energy Conversion", "HVAC Systems"],
  related_fields = ["engineering:energy", "engineering:materials"];

CREATE mechanical_engineering:fluid_mechanics SET
  name = "Fluid Mechanics",
  description = "The branch of physics concerned with the mechanics of fluids and the forces on them",
  type = "Field",
  components = ["Pumps", "Turbines"],
  applications = ["Hydraulics", "Aerodynamics"],
  related_fields = ["engineering:aerospace", "engineering:environmental"];

CREATE mechanical_engineering:materials_science SET
  name = "Materials Science",
  description = "The study of the properties of solid materials and how those properties are determined by a material's composition and structure",
  type = "Field",
  components = ["Metals", "Polymers"],
  applications = ["Material Design", "Failure Analysis"],
  related_fields = ["engineering:manufacturing", "engineering:chemistry"];
