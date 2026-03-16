USE NS project_universe;
USE DB economy;

DEFINE TABLE real_services SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE real_services TYPE string;
DEFINE FIELD description     ON TABLE real_services TYPE string;
DEFINE FIELD category        ON TABLE real_services TYPE string;
DEFINE FIELD cost            ON TABLE real_services TYPE float;
DEFINE FIELD market_demand   ON TABLE real_services TYPE float;
DEFINE FIELD related_fields  ON TABLE real_services TYPE array;

DEFINE INDEX unique_real_services_name ON TABLE real_services COLUMNS name UNIQUE;

-- Example real services
CREATE real_services:healthcare SET
  name = "Healthcare",
  description = "Medical services provided to individuals or communities",
  category = "Health",
  cost = 500.0,
  market_demand = 0.8,
  related_fields = ["economy:public_health", "economy:insurance"];

CREATE real_services:education SET
  name = "Education",
  description = "Services related to teaching and learning in schools or universities",
  category = "Education",
  cost = 1000.0,
  market_demand = 0.9,
  related_fields = ["economy:public_policy", "economy:human_resources"];

CREATE real_services:transportation SET
  name = "Transportation",
  description = "Services that move people or goods from one place to another",
  category = "Logistics",
  cost = 200.0,
  market_demand = 0.7,
  related_fields = ["economy:infrastructure", "economy:logistics"];
