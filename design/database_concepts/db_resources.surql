USE NS project_universe;
USE DB economy;

DEFINE TABLE economy SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE economy TYPE string;
DEFINE FIELD description     ON TABLE economy TYPE string;
DEFINE FIELD type            ON TABLE economy TYPE string;
DEFINE FIELD sectors         ON TABLE economy TYPE array;
DEFINE FIELD indicators      ON TABLE economy TYPE array;
DEFINE FIELD policies        ON TABLE economy TYPE array;
DEFINE FIELD related_fields  ON TABLE economy TYPE array;

DEFINE INDEX unique_economy_name ON TABLE economy COLUMNS name UNIQUE;

-- Example economic concepts
CREATE economy:capitalism SET
  name = "Capitalism",
  description = "An economic system based on private ownership of the means of production and their operation for profit",
  type = "Economic System",
  sectors = ["Private Sector", "Market Economy"],
  indicators = ["GDP Growth", "Market Competition"],
  policies = ["Deregulation", "Free Trade"],
  related_fields = ["economy:finance", "economy:business"];

CREATE economy:socialism SET
  name = "Socialism",
  description = "An economic system where the means of production are owned and regulated by the community as a whole",
  type = "Economic System",
  sectors = ["Public Sector", "Planned Economy"],
  indicators = ["Social Welfare", "Economic Equality"],
  policies = ["Nationalization", "Social Programs"],
  related_fields = ["economy:public_policy", "economy:law"];

CREATE economy:mixed_economy SET
  name = "Mixed Economy",
  description = "An economic system combining private and public enterprise",
  type = "Economic System",
  sectors = ["Private Sector", "Public Sector"],
  indicators = ["Balanced Growth", "Regulatory Framework"],
  policies = ["Public-Private Partnerships", "Regulation"],
  related_fields = ["economy:business", "economy:government"];
