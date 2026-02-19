USE NS project_universe;
USE DB science;
  
DEFINE TABLE geology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE geology TYPE string;
DEFINE FIELD description     ON TABLE geology TYPE string;
DEFINE FIELD type           ON TABLE geology TYPE string;
DEFINE FIELD composition     ON TABLE geology TYPE array;
DEFINE FIELD formation      ON TABLE geology TYPE string;
DEFINE FIELD properties     ON TABLE geology TYPE array;
DEFINE FIELD hazards        ON TABLE geology TYPE array;
DEFINE FIELD resources      ON TABLE geology TYPE array;
DEFINE FIELD related_materials ON TABLE geology TYPE array;
DEFINE FIELD physical_properties ON TABLE geology TYPE object;
DEFINE FIELD chemical_properties ON TABLE geology TYPE object;
  
DEFINE INDEX unique_geological_material ON TABLE geology COLUMNS name UNIQUE;
DEFINE INDEX material_type ON TABLE geology COLUMNS type;
DEFINE INDEX physical_properties ON TABLE geology COLUMNS physical_properties;
DEFINE INDEX chemical_properties ON TABLE geology COLUMNS chemical_properties;
  
-- Minerals and Rocks
CREATE geology:iron_ore SET
  name = "Iron Ore",
  description = "Rock containing iron minerals in sufficient quantity to be economically extracted",
  type = "Metallic ore",
  composition = ["Hematite", "Magnetite", "Goethite", "Siderite"],
  formation = "Sedimentary, metamorphic, or igneous processes",
  properties = ["Magnetic", "Dense", "Hard", "Red to black color"],
  hazards = ["Heavy", "Sharp edges", "Dust inhalation"],
  resources = ["Iron", "Steel production", "Construction"],
  related_materials = ["geology:hematite", "geology:magnetite", "geology:limestone"],
  physical_properties = {
    hardness: "5-6.5",
    density: "4.8-5.3 g/cm³",
    color: "Red to black",
    luster: "Metallic to dull"
  },
  chemical_properties = {
    formula: "Fe₂O₃, Fe₃O₄",
    solubility: "Insoluble in water",
    reactivity: "Oxidizes in air",
    melting_point: "1538°C"
  };
  
CREATE geology:copper_ore SET
  name = "Copper Ore",
  description = "Rock containing copper minerals in economically extractable amounts",
  type = "Metallic ore",
  composition = ["Chalcopyrite", "Bornite", "Malachite", "Azurite"],
  formation = "Hydrothermal, sedimentary, or metamorphic processes",
  properties = ["Conductive", "Malleable", "Green to blue color"],
  hazards = ["Toxic when processed", "Heavy", "Sharp edges"],
  resources = ["Copper", "Electrical wiring", "Alloys"],
  related_materials = ["geology:chalcopyrite", "geology:malachite", "geology:quartz"],
  physical_properties = {
    hardness: "3-4",
    density: "4.1-4.3 g/cm³",
    color: "Green to blue",
    luster: "Metallic to vitreous"
  },
  chemical_properties = {
    formula: "CuFeS₂, Cu₅FeS₄",
    solubility: "Insoluble in water",
    reactivity: "Oxidizes in air",
    melting_point: "1085°C"
  };
  
CREATE geology:water_ice SET
  name = "Water Ice",
  description = "Solid form of water, commonly found in space environments",
  type = "Volatile",
  composition = ["H₂O", "Traces of other volatiles"],
  formation = "Freezing of water or condensation of water vapor",
  properties = ["Brittle", "Low density", "Transparent to opaque"],
  hazards = ["Slippery", "Expansion on melting", "Low temperature"],
  resources = ["Water", "Oxygen", "Hydrogen"],
  related_materials = ["geology:dry_ice", "geology:ammonia_ice", "geology:methane_ice"],
  physical_properties = {
    hardness: "1.5",
    density: "0.917 g/cm³",
    color: "White to transparent",
    melting_point: "0°C"
  },
  chemical_properties = {
    formula: "H₂O",
    solubility: "Soluble in water",
    reactivity: "Stable at low temperatures",
    phase: "Solid"
  };
  
CREATE geology:regolith SET
  name = "Regolith",
  description = "Layer of loose, heterogeneous material covering solid rock",
  type = "Sediment",
  composition = ["Rock fragments", "Mineral grains", "Glass", "Metal particles"],
  formation = "Weathering, impact processes, and space weathering",
  properties = ["Fine-grained", "Porous", "Electrically charged"],
  hazards = ["Sharp particles", "Dust inhalation", "Electrostatic"],
  resources = ["Building materials", "Oxygen extraction", "Metal extraction"],
  related_materials = ["geology:soil", "geology:breccia", "geology:impact_glass"],
  physical_properties = {
    grain_size: "Microns to centimeters",
    porosity: "30-50%",
    density: "1.5-2.0 g/cm³",
    cohesion: "Low to none"
  },
  chemical_properties = {
    composition: "Variable",
    reactivity: "High surface area",
    weathering: "Space weathering effects",
    alteration: "Solar wind interaction"
  };
  
CREATE geology:volcanic_glass SET
  name = "Volcanic Glass",
  description = "Amorphous solid formed from rapidly cooled lava",
  type = "Igneous",
  composition = ["Silica", "Aluminum", "Iron", "Magnesium"],
  formation = "Rapid cooling of lava",
  properties = ["Sharp edges", "Conchoidal fracture", "Glassy luster"],
  hazards = ["Sharp edges", "Brittle", "Heat retention"],
  resources = ["Tool making", "Construction", "Fiber optics"],
  related_materials = ["geology:obsidian", "geology:pumice", "geology:basalt"],
  physical_properties = {
    hardness: "5-6",
    density: "2.3-2.5 g/cm³",
    color: "Black to dark brown",
    fracture: "Conchoidal"
  },
  chemical_properties = {
    composition: "70-75% SiO₂",
    solubility: "Insoluble in water",
    reactivity: "Low",
    melting_point: "800-1200°C"
  };
