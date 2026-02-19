USE NS project_universe;
USE DB economy;

DEFINE TABLE virtual_services SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE virtual_services TYPE string;
DEFINE FIELD description     ON TABLE virtual_services TYPE string;
DEFINE FIELD category        ON TABLE virtual_services TYPE string;
DEFINE FIELD cost            ON TABLE virtual_services TYPE float;
DEFINE FIELD market_demand   ON TABLE virtual_services TYPE float;
DEFINE FIELD related_fields  ON TABLE virtual_services TYPE array;

DEFINE INDEX unique_virtual_services_name ON TABLE virtual_services COLUMNS name UNIQUE;

-- Example virtual services
CREATE virtual_services:cloud_storage SET
  name = "Cloud Storage",
  description = "A service model in which data is maintained, managed, and backed up remotely and made available to users over a network",
  category = "Data Services",
  cost = 50.0,
  market_demand = 0.9,
  related_fields = ["economy:technology", "economy:internet"];

CREATE virtual_services:streaming SET
  name = "Streaming",
  description = "A method of transmitting or receiving data (especially video and audio material) over a computer network as a steady, continuous flow",
  category = "Media Services",
  cost = 15.0,
  market_demand = 0.95,
  related_fields = ["economy:media", "economy:entertainment"];

CREATE virtual_services:online_education SET
  name = "Online Education",
  description = "Education that takes place over the Internet. It is often referred to as e-learning among other terms",
  category = "Educational Services",
  cost = 100.0,
  market_demand = 0.85,
  related_fields = ["economy:education", "economy:technology"];
