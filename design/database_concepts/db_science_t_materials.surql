USE NS project_universe;
USE DB science;

DEFINE TABLE materials SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE materials TYPE string;
DEFINE FIELD description     ON TABLE materials TYPE string;
DEFINE FIELD type            ON TABLE materials TYPE string;
DEFINE FIELD composition     ON TABLE materials TYPE array;
DEFINE FIELD properties      ON TABLE materials TYPE object;
DEFINE FIELD applications    ON TABLE materials TYPE array;
DEFINE FIELD limitations     ON TABLE materials TYPE array;
DEFINE FIELD related_systems ON TABLE materials TYPE array;

DEFINE INDEX unique_material_name ON TABLE materials COLUMNS name UNIQUE;

-- Example materials
CREATE materials:carbon_nanotubes SET
  name = "Carbon Nanotubes",
  description = "Cylindrical nanostructures with exceptional strength and electrical properties",
  type = "Nanomaterial",
  composition = ["Carbon"],
  properties = {strength: "100 times stronger than steel", conductivity: "Excellent"},
  applications = ["Electronics", "Composite Materials", "Energy Storage"],
  limitations = ["Cost", "Scalability"],
  related_systems = ["science:nanotechnology", "technology:electronics"];

CREATE materials:graphene SET
  name = "Graphene",
  description = "Single layer of carbon atoms with remarkable electrical and mechanical properties",
  type = "Nanomaterial",
  composition = ["Carbon"],
  properties = {conductivity: "High", flexibility: "Excellent"},
  applications = ["Flexible Electronics", "Conductive Inks", "Sensors"],
  limitations = ["Production Challenges", "Cost"],
  related_systems = ["science:nanotechnology", "technology:electronics"];

CREATE materials:titanium_alloy SET
  name = "Titanium Alloy",
  description = "Alloy known for its high strength, low density, and corrosion resistance",
  type = "Metal Alloy",
  composition = ["Titanium", "Aluminum", "Vanadium"],
  properties = {strength: "High", weight: "Light"},
  applications = ["Aerospace", "Medical Implants", "Automotive"],
  limitations = ["Cost", "Machinability"],
  related_systems = ["science:metallurgy", "technology:aerospace"];

CREATE materials:ceramic_composite SET
  name = "Ceramic Composite",
  description = "Composite material made from ceramic fibers embedded in a ceramic matrix",
  type = "Composite",
  composition = ["Ceramic Fibers", "Ceramic Matrix"],
  properties = {heat_resistance: "High", brittleness: "Moderate"},
  applications = ["Heat Shields", "Cutting Tools", "Armor"],
  limitations = ["Brittleness", "Cost"],
  related_systems = ["science:materials_science", "technology:defense"];
