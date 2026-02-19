USE NS project_universe;
USE DB lifeforms;
  
DEFINE TABLE behavior SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD species_id      ON TABLE behavior TYPE string;
DEFINE FIELD social_structure ON TABLE behavior TYPE object;
DEFINE FIELD communication   ON TABLE behavior TYPE array;
DEFINE FIELD daily_patterns  ON TABLE behavior TYPE array;
DEFINE FIELD seasonal_patterns ON TABLE behavior TYPE array;
DEFINE FIELD learning        ON TABLE behavior TYPE object;
DEFINE FIELD instincts       ON TABLE behavior TYPE array;
DEFINE FIELD adaptations     ON TABLE behavior TYPE array;
DEFINE FIELD related_systems ON TABLE behavior TYPE array;
  
DEFINE INDEX species_behavior ON TABLE behavior COLUMNS species_id;
  
-- Human Behavior
CREATE behavior:human_behavior SET
  species_id = "species:human",
  social_structure = {
    type: "Complex hierarchical",
    group_size: "Variable",
    organization: "Highly structured",
    hierarchy: "Multiple levels"
  },
  communication = [
    "Verbal language",
    "Written language",
    "Body language",
    "Facial expressions",
    "Technology-mediated",
    "Art and music"
  ],
  daily_patterns = [
    "Sleep-wake cycle",
    "Work-rest periods",
    "Social interactions",
    "Meal times",
    "Personal care"
  ],
  seasonal_patterns = [
    "Cultural celebrations",
    "Agricultural cycles",
    "Social gatherings",
    "Clothing adaptation",
    "Activity changes"
  ],
  learning = {
    type: "Complex cognitive",
    methods: ["Observation", "Instruction", "Experience", "Technology"],
    capacity: "Very high",
    transmission: "Cultural"
  },
  instincts = [
    "Self-preservation",
    "Social bonding",
    "Reproduction",
    "Territoriality",
    "Basic survival"
  ],
  adaptations = [
    "Cultural adaptation",
    "Technological use",
    "Social cooperation",
    "Environmental modification",
    "Language development"
  ],
  related_systems = ["physiology:human_physiology", "ecology:human_ecology"];
  
-- Sheep Behavior
CREATE behavior:sheep_behavior SET
  species_id = "species:sheep",
  social_structure = {
    type: "Flock-based",
    group_size: "5-100",
    organization: "Hierarchical",
    hierarchy: "Dominance-based"
  },
  communication = [
    "Vocalizations",
    "Body language",
    "Scent marking",
    "Visual signals",
    "Physical contact"
  ],
  daily_patterns = [
    "Grazing periods",
    "Rest periods",
    "Social grooming",
    "Movement patterns",
    "Feeding times"
  ],
  seasonal_patterns = [
    "Breeding season",
    "Wool growth",
    "Migration",
    "Social changes",
    "Activity levels"
  ],
  learning = {
    type: "Social learning",
    methods: ["Observation", "Experience", "Imitation"],
    capacity: "Moderate",
    transmission: "Social"
  },
  instincts = [
    "Flocking",
    "Grazing",
    "Fleeing",
    "Maternal care",
    "Territorial marking"
  ],
  adaptations = [
    "Group defense",
    "Resource sharing",
    "Environmental awareness",
    "Social coordination",
    "Predator avoidance"
  ],
  related_systems = ["physiology:sheep_physiology", "ecology:sheep_ecology"];
  
-- Apple Behavior
CREATE behavior:apple_behavior SET
  species_id = "species:apple",
  social_structure = {
    type: "Population-based",
    distribution: "Clustered",
    organization: "Resource-based",
    spacing: "Variable"
  },
  communication = [
    "Chemical signals",
    "Root interactions",
    "Pollen dispersal",
    "Fruit signals",
    "Environmental response"
  ],
  daily_patterns = [
    "Photosynthesis cycles",
    "Water uptake",
    "Growth patterns",
    "Leaf movements",
    "Root activity"
  ],
  seasonal_patterns = [
    "Flowering",
    "Fruit production",
    "Leaf shedding",
    "Dormancy",
    "Growth cycles"
  ],
  learning = {
    type: "Environmental adaptation",
    methods: ["Environmental response", "Resource optimization"],
    capacity: "Basic",
    transmission: "Genetic"
  },
  instincts = [
    "Root growth",
    "Branch development",
    "Flower production",
    "Fruit development",
    "Seed dispersal"
  ],
  adaptations = [
    "Seasonal changes",
    "Resource allocation",
    "Defense mechanisms",
    "Reproductive timing",
    "Environmental response"
  ],
  related_systems = ["physiology:apple_physiology", "ecology:apple_ecology"]; 