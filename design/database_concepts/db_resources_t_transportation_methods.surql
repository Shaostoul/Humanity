USE NS project_universe;
USE DB economy;

DEFINE TABLE transportation_methods SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE transportation_methods TYPE string;
DEFINE FIELD description     ON TABLE transportation_methods TYPE string;
DEFINE FIELD category        ON TABLE transportation_methods TYPE string;
DEFINE FIELD operational_cost ON TABLE transportation_methods TYPE float;
DEFINE FIELD market_value    ON TABLE transportation_methods TYPE float;
DEFINE FIELD related_fields  ON TABLE transportation_methods TYPE array;

DEFINE INDEX unique_transportation_methods_name ON TABLE transportation_methods COLUMNS name UNIQUE;

-- Example transportation methods
CREATE transportation_methods:rail_transport SET
  name = "Rail Transport",
  description = "Transportation of goods and passengers on railways",
  category = "Land",
  operational_cost = 30.0,
  market_value = 70.0,
  related_fields = ["economy:logistics", "economy:infrastructure"];

CREATE transportation_methods:air_transport SET
  name = "Air Transport",
  description = "Transportation of goods and passengers by aircraft",
  category = "Air",
  operational_cost = 50.0,
  market_value = 100.0,
  related_fields = ["economy:logistics", "economy:aviation"];

CREATE transportation_methods:sea_transport SET
  name = "Sea Transport",
  description = "Transportation of goods and passengers by sea vessels",
  category = "Water",
  operational_cost = 40.0,
  market_value = 80.0,
  related_fields = ["economy:logistics", "economy:maritime"]; 