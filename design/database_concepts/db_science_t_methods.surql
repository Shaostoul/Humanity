USE NS project_universe;
USE DB lifeforms;

DEFINE TABLE methods SCHEMAFULL
  PERMISSIONS
    FOR create WHERE $auth.role == "admin"
    FOR select WHERE true
    FOR update WHERE $auth.role == "admin"
    FOR delete WHERE $auth.role == "admin";

DEFINE FIELD action_id           ON TABLE methods TYPE record(actions) ASSERT $value != NONE;
DEFINE FIELD name                ON TABLE methods TYPE string ASSERT $value != NONE;
DEFINE FIELD type                ON TABLE methods TYPE string ASSERT $value INSIDE ["manual", "mechanical", "chemical", "electrical", "thermal", "biological"];
DEFINE FIELD equipment_required  ON TABLE methods TYPE array;
DEFINE FIELD efficiency          ON TABLE methods TYPE float;
DEFINE FIELD labor_intensity     ON TABLE methods TYPE int;
DEFINE FIELD duration            ON TABLE methods TYPE duration;
DEFINE FIELD conditions          ON TABLE methods TYPE object;
DEFINE FIELD notes               ON TABLE methods TYPE string;

DEFINE INDEX idx_methods_name ON TABLE methods COLUMNS name UNIQUE;

-- Examples of Methods

CREATE methods SET
  action_id = actions:cutting,
  name = "Hand Saw Cutting",
  type = "manual",
  equipment_required = [items:hand_saw],
  efficiency = 0.2,
  labor_intensity = 8,
  duration = 30m,
  conditions = { "skills": "Basic manual dexterity", "safety": "Gloves recommended" },
  notes = "Physically demanding, suitable for small-scale operations.";

CREATE methods SET
  action_id = actions:cutting,
  name = "Laser Cutting",
  type = "mechanical",
  equipment_required = [items:laser_cutter_machine],
  efficiency = 1.0,
  labor_intensity = 2,
  duration = 5m,
  conditions = { "electricity": "High power", "safety": "Eye protection required" },
  notes = "Highly precise, efficient, suitable for mass production.";

CREATE methods SET
  action_id = actions:shredding,
  name = "Manual Plastic Shredding",
  type = "manual",
  equipment_required = [items:heavy_duty_scissors, items:workbench],
  efficiency = 0.15,
  labor_intensity = 9,
  duration = 1h,
  conditions = { "skills": "Endurance, basic dexterity", "safety": "Gloves recommended" },
  notes = "Low yield, physically demanding.";

CREATE methods SET
  action_id = actions:shredding,
  name = "Industrial Plastic Shredding",
  type = "mechanical",
  equipment_required = [items:plastic_shredder_machine],
  efficiency = 1.0,
  labor_intensity = 2,
  duration = 10m,
  conditions = { "electricity": "Required", "safety": "Protective gear necessary" },
  notes = "Fast, efficient, suitable for large-scale recycling.";

CREATE methods SET
  action_id = actions:electrolysis,
  name = "Manual Electrolysis",
  type = "electrical",
  equipment_required = [items:basic_electrolysis_setup],
  efficiency = 0.5,
  labor_intensity = 5,
  duration = 4h,
  conditions = { "electricity": "Moderate", "safety": "Chemical handling precautions" },
  notes = "Suitable for small-scale precious metal recovery.";

CREATE methods SET
  action_id = actions:electrolysis,
  name = "Automated Electrolysis",
  type = "electrical",
  equipment_required = [items:industrial_electrolysis_rig],
  efficiency = 0.95,
  labor_intensity = 1,
  duration = 1h,
  conditions = { "electricity": "High", "safety": "Automated safety systems" },
  notes = "High yield, efficient, ideal for commercial-scale operations.";
