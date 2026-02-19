USE NS project_universe;
USE DB lifeforms;
  
DEFINE TABLE physiology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD species_id      ON TABLE physiology TYPE string;
DEFINE FIELD body_structure  ON TABLE physiology TYPE object;
DEFINE FIELD organ_systems   ON TABLE physiology TYPE array;
DEFINE FIELD metabolism      ON TABLE physiology TYPE object;
DEFINE FIELD growth_stages   ON TABLE physiology TYPE array;
DEFINE FIELD physical_limits ON TABLE physiology TYPE object;
DEFINE FIELD adaptations     ON TABLE physiology TYPE array;
DEFINE FIELD health_factors  ON TABLE physiology TYPE array;
DEFINE FIELD related_systems ON TABLE physiology TYPE array;
  
DEFINE INDEX species_physiology ON TABLE physiology COLUMNS species_id;
  
-- Human Physiology
CREATE physiology:human_physiology SET
  species_id = "species:human",
  body_structure = {
    height: "150-200 cm",
    weight: "50-100 kg",
    body_type: "Bipedal",
    symmetry: "Bilateral",
    skeleton: "Endoskeleton"
  },
  organ_systems = [
    "Nervous System",
    "Circulatory System",
    "Respiratory System",
    "Digestive System",
    "Muscular System",
    "Skeletal System",
    "Endocrine System",
    "Immune System",
    "Reproductive System"
  ],
  metabolism = {
    type: "Endothermic",
    temperature: "37°C",
    energy_source: "Heterotrophic"
  },
  growth_stages = [
    "Infant",
    "Child",
    "Adolescent",
    "Adult",
    "Elderly"
  ],
  physical_limits = {
    temperature: "20-40°C",
    oxygen: "Required",
    water: "2-3L daily",
    food: "2000-3000 kcal daily"
  },
  adaptations = [
    "Bipedal locomotion",
    "Opposable thumbs",
    "Large brain",
    "Complex vocal apparatus"
  ],
  health_factors = [
    "Nutrition",
    "Exercise",
    "Sleep",
    "Mental health",
    "Environmental conditions"
  ],
  related_systems = ["behavior:human_behavior", "ecology:human_ecology"];
  
-- Sheep Physiology
CREATE physiology:sheep_physiology SET
  species_id = "species:sheep",
  body_structure = {
    height: "60-100 cm",
    weight: "45-160 kg",
    body_type: "Quadrupedal",
    symmetry: "Bilateral",
    skeleton: "Endoskeleton"
  },
  organ_systems = [
    "Nervous System",
    "Circulatory System",
    "Respiratory System",
    "Digestive System",
    "Muscular System",
    "Skeletal System",
    "Endocrine System",
    "Immune System",
    "Reproductive System"
  ],
  metabolism = {
    type: "Endothermic",
    temperature: "38.5-40°C",
    energy_source: "Heterotrophic"
  },
  growth_stages = [
    "Lamb",
    "Yearling",
    "Adult",
    "Senior"
  ],
  physical_limits = {
    temperature: "5-30°C",
    oxygen: "Required",
    water: "2-4L daily",
    food: "2-4% body weight daily"
  },
  adaptations = [
    "Ruminant digestion",
    "Wool coat",
    "Split hooves",
    "Rectangular pupils"
  ],
  health_factors = [
    "Nutrition",
    "Parasite control",
    "Hoof care",
    "Wool management",
    "Environmental conditions"
  ],
  related_systems = ["behavior:sheep_behavior", "ecology:sheep_ecology"];
  
-- Apple Physiology
CREATE physiology:apple_physiology SET
  species_id = "species:apple",
  body_structure = {
    height: "2-12 m",
    spread: "3-15 m",
    growth_form: "Tree",
    symmetry: "Radial",
    structure: "Woody"
  },
  organ_systems = [
    "Root System",
    "Stem System",
    "Leaf System",
    "Flower System",
    "Fruit System"
  ],
  metabolism = {
    type: "Autotrophic",
    energy_source: "Photosynthesis",
    temperature: "15-30°C optimal"
  },
  growth_stages = [
    "Seed",
    "Seedling",
    "Sapling",
    "Mature tree",
    "Senescent"
  ],
  physical_limits = {
    temperature: "-20 to 35°C",
    water: "Regular irrigation",
    soil: "Well-drained, fertile",
    light: "Full sun"
  },
  adaptations = [
    "Deciduous leaves",
    "Deep root system",
    "Flower pollination",
    "Fruit dispersal"
  ],
  health_factors = [
    "Soil quality",
    "Water availability",
    "Sunlight",
    "Disease resistance",
    "Pest control"
  ],
  related_systems = ["behavior:apple_behavior", "ecology:apple_ecology"]; 