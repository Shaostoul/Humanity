USE NS project_universe;
USE DB science;

DEFINE TABLE chemistry SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE chemistry TYPE string;
DEFINE FIELD description     ON TABLE chemistry TYPE string;
DEFINE FIELD reaction        ON TABLE chemistry TYPE string;
DEFINE FIELD reactants       ON TABLE chemistry TYPE array;
DEFINE FIELD products        ON TABLE chemistry TYPE array;
DEFINE FIELD conditions      ON TABLE chemistry TYPE array;
DEFINE FIELD applications    ON TABLE chemistry TYPE array;
DEFINE FIELD hazards         ON TABLE chemistry TYPE array;
DEFINE FIELD related_reactions ON TABLE chemistry TYPE array;

DEFINE INDEX unique_chemical_reaction ON TABLE chemistry COLUMNS name UNIQUE;

-- Life Support Reactions
CREATE chemistry:water_electrolysis SET
  name = "Water Electrolysis",
  description = "Splitting water into hydrogen and oxygen using electricity",
  reaction = "2H₂O(l) → 2H₂(g) + O₂(g)",
  reactants = ["H₂O"],
  products = ["H₂", "O₂"],
  conditions = ["Electric current", "Electrolyte"],
  applications = ["Oxygen Generation", "Hydrogen Fuel Production"],
  hazards = ["Flammable hydrogen gas", "High voltage"],
  related_reactions = ["chemistry:hydrogen_combustion"];

CREATE chemistry:hydrogen_combustion SET
  name = "Hydrogen Combustion",
  description = "Burning hydrogen with oxygen to produce water and energy",
  reaction = "2H₂(g) + O₂(g) → 2H₂O(l) + Energy",
  reactants = ["H₂", "O₂"],
  products = ["H₂O", "Energy"],
  conditions = ["Heat", "Catalyst"],
  applications = ["Rocket Propulsion", "Power Generation"],
  hazards = ["Explosive reaction", "High temperature"],
  related_reactions = ["chemistry:water_electrolysis"];

-- Carbon Cycle Reactions
CREATE chemistry:photosynthesis SET
  name = "Photosynthesis",
  description = "Conversion of carbon dioxide and water into glucose using light energy",
  reaction = "6CO₂ + 6H₂O + Light → C₆H₁₂O₆ + 6O₂",
  reactants = ["CO₂", "H₂O", "Light"],
  products = ["C₆H₁₂O₆", "O₂"],
  conditions = ["Sunlight", "Chlorophyll"],
  applications = ["Life Support Systems", "Food Production"],
  hazards = ["None"],
  related_reactions = ["chemistry:cellular_respiration"];

CREATE chemistry:cellular_respiration SET
  name = "Cellular Respiration",
  description = "Breakdown of glucose to produce energy, CO₂, and water",
  reaction = "C₆H₁₂O₆ + 6O₂ → 6CO₂ + 6H₂O + Energy",
  reactants = ["C₆H₁₂O₆", "O₂"],
  products = ["CO₂", "H₂O", "Energy"],
  conditions = ["Enzymes", "Oxygen"],
  applications = ["Energy Production", "CO₂ Generation"],
  hazards = ["None"],
  related_reactions = ["chemistry:photosynthesis"];

-- Resource Processing
CREATE chemistry:iron_oxidation SET
  name = "Iron Oxidation",
  description = "Formation of iron oxide from iron and oxygen",
  reaction = "4Fe + 3O₂ → 2Fe₂O₃",
  reactants = ["Fe", "O₂"],
  products = ["Fe₂O₃"],
  conditions = ["Moisture", "Oxygen"],
  applications = ["Metal Processing", "Resource Extraction"],
  hazards = ["Corrosive", "Heat generation"],
  related_reactions = ["chemistry:iron_reduction"];

CREATE chemistry:iron_reduction SET
  name = "Iron Reduction",
  description = "Reduction of iron oxide to metallic iron",
  reaction = "Fe₂O₃ + 3CO → 2Fe + 3CO₂",
  reactants = ["Fe₂O₃", "CO"],
  products = ["Fe", "CO₂"],
  conditions = ["High temperature", "Carbon monoxide"],
  applications = ["Metal Production", "Resource Processing"],
  hazards = ["Toxic CO", "High temperature"],
  related_reactions = ["chemistry:iron_oxidation"];

-- Waste Processing
CREATE chemistry:ammonia_synthesis SET
  name = "Ammonia Synthesis",
  description = "Production of ammonia from nitrogen and hydrogen",
  reaction = "N₂ + 3H₂ → 2NH₃",
  reactants = ["N₂", "H₂"],
  products = ["NH₃"],
  conditions = ["High pressure", "Catalyst", "High temperature"],
  applications = ["Fertilizer Production", "Waste Processing"],
  hazards = ["High pressure", "Toxic product"],
  related_reactions = ["chemistry:ammonia_decomposition"];

CREATE chemistry:ammonia_decomposition SET
  name = "Ammonia Decomposition",
  description = "Breakdown of ammonia into nitrogen and hydrogen",
  reaction = "2NH₃ → N₂ + 3H₂",
  reactants = ["NH₃"],
  products = ["N₂", "H₂"],
  conditions = ["Heat", "Catalyst"],
  applications = ["Waste Processing", "Hydrogen Production"],
  hazards = ["Toxic reactant", "High temperature"],
  related_reactions = ["chemistry:ammonia_synthesis"];
