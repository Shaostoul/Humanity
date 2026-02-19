USE NS project_universe;
USE DB lifeforms;

DEFINE TABLE processes SCHEMAFULL
  PERMISSIONS
    FOR create WHERE $auth.role == "admin"
    FOR select WHERE true
    FOR update WHERE $auth.role == "admin"
    FOR delete WHERE $auth.role == "admin";

DEFINE FIELD name              ON TABLE processes TYPE string ASSERT $value != NONE;
DEFINE FIELD description       ON TABLE processes TYPE string ASSERT $value != NONE;
DEFINE FIELD inputs            ON TABLE processes TYPE array;
DEFINE FIELD outputs           ON TABLE processes TYPE array;
DEFINE FIELD actions           ON TABLE processes TYPE array;
DEFINE FIELD total_duration    ON TABLE processes TYPE duration;
DEFINE FIELD notes             ON TABLE processes TYPE string;

DEFINE INDEX idx_processes_name ON TABLE processes COLUMNS name UNIQUE;

-- Example Processes

CREATE processes SET
  name = "PET Bottle to Filament (Shredding Method)",
  description = "Converts PET bottles into usable 3D printing filament by shredding, drying, melting, and extruding.",
  inputs = [materials:pet_bottle],
  outputs = [materials:pet_filament],
  actions = [actions:shredding, actions:drying, actions:melting, actions:extruding],
  total_duration = 3h,
  notes = "Highly efficient, produces clean filament, suitable for large-scale recycling.";

CREATE processes SET
  name = "PET Bottle to Filament (Stripping Method)",
  description = "Converts PET bottles directly into filament by stripping and extruding, without shredding.",
  inputs = [materials:pet_bottle],
  outputs = [materials:pet_filament],
  actions = [actions:stripping, actions:melting, actions:extruding],
  total_duration = 2h,
  notes = "Faster but slightly less efficient due to leftover scraps.";

CREATE processes SET
  name = "Gold Recovery from Circuit Boards",
  description = "Extracts gold from electronic circuit boards via chemical dissolution and electrolysis.",
  inputs = [materials:circuit_boards],
  outputs = [materials:pure_gold, materials:electronic_scrap],
  actions = [actions:dissolving, actions:filtering, actions:electrolysis],
  total_duration = 6h,
  notes = "Chemically intensive, requires careful handling and waste management.";

CREATE processes SET
  name = "Organic Composting",
  description = "Transforms organic waste into nutrient-rich compost through biological decomposition.",
  inputs = [materials:organic_waste],
  outputs = [materials:compost],
  actions = [actions:composting],
  total_duration = 720h,
  notes = "Natural process, duration varies significantly based on environmental conditions.";
