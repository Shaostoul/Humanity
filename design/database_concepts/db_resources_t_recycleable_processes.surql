USE NS project_universe;
USE DB economy;

DEFINE TABLE recycleable_processes SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE recycleable_processes TYPE string;
DEFINE FIELD description     ON TABLE recycleable_processes TYPE string;
DEFINE FIELD category        ON TABLE recycleable_processes TYPE string;
DEFINE FIELD process_cost    ON TABLE recycleable_processes TYPE float;
DEFINE FIELD efficiency      ON TABLE recycleable_processes TYPE float;
DEFINE FIELD related_fields  ON TABLE recycleable_processes TYPE array;

DEFINE INDEX unique_recycleable_processes_name ON TABLE recycleable_processes COLUMNS name UNIQUE;

-- Example recyclable processes
CREATE recycleable_processes:plastic_recycling SET
  name = "Plastic Recycling",
  description = "Process of recovering scrap or waste plastic and reprocessing it into useful products",
  category = "Material Recovery",
  process_cost = 100.0,
  efficiency = 0.85,
  related_fields = ["economy:waste_management", "economy:manufacturing"];

CREATE recycleable_processes:metal_recycling SET
  name = "Metal Recycling",
  description = "Process of recovering metal from scrap or waste and reprocessing it into useful products",
  category = "Material Recovery",
  process_cost = 150.0,
  efficiency = 0.9,
  related_fields = ["economy:waste_management", "economy:manufacturing"];

CREATE recycleable_processes:glass_recycling SET
  name = "Glass Recycling",
  description = "Process of converting waste glass into usable products",
  category = "Material Recovery",
  process_cost = 80.0,
  efficiency = 0.95,
  related_fields = ["economy:waste_management", "economy:manufacturing"];
