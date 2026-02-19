USE NS project_universe;
USE DB lifeforms;
  
DEFINE TABLE species SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE species TYPE string;
DEFINE FIELD scientific_name ON TABLE species TYPE string;
DEFINE FIELD description     ON TABLE species TYPE string;
DEFINE FIELD kingdom        ON TABLE species TYPE string;
DEFINE FIELD phylum         ON TABLE species TYPE string;
DEFINE FIELD class          ON TABLE species TYPE string;
DEFINE FIELD order          ON TABLE species TYPE string;
DEFINE FIELD family         ON TABLE species TYPE string;
DEFINE FIELD genus          ON TABLE species TYPE string;
DEFINE FIELD species        ON TABLE species TYPE string;
DEFINE FIELD characteristics ON TABLE species TYPE object;
DEFINE FIELD habitat        ON TABLE species TYPE array;
DEFINE FIELD diet           ON TABLE species TYPE array;
DEFINE FIELD reproduction   ON TABLE species TYPE string;
DEFINE FIELD lifespan       ON TABLE species TYPE object;
DEFINE FIELD status         ON TABLE species TYPE string;
DEFINE FIELD related_species ON TABLE species TYPE array;
  
DEFINE INDEX unique_species_name ON TABLE species COLUMNS name UNIQUE;
DEFINE INDEX scientific_name ON TABLE species COLUMNS scientific_name UNIQUE;
  
-- Human
CREATE species:human SET
  name = "Human",
  scientific_name = "Homo sapiens",
  description = "Intelligent bipedal primate species with advanced cognitive abilities",
  kingdom = "Animalia",
  phylum = "Chordata",
  class = "Mammalia",
  order = "Primates",
  family = "Hominidae",
  genus = "Homo",
  species = "sapiens",
  characteristics = {
    intelligence: "High",
    adaptability: "Very high",
    social_complexity: "Very high",
    tool_use: "Advanced"
  },
  habitat = ["Terrestrial", "Urban", "Rural", "Arctic", "Desert", "Tropical"],
  diet = ["Omnivorous", "Cultivated foods", "Meat", "Plants"],
  reproduction = "Sexual",
  lifespan = {
    average: "70-80 years",
    maximum: "120 years"
  },
  status = "Least Concern",
  related_species = ["species:sheep"];
  
-- Sheep
CREATE species:sheep SET
  name = "Sheep",
  scientific_name = "Ovis aries",
  description = "Domesticated ruminant mammal valued for wool and meat",
  kingdom = "Animalia",
  phylum = "Chordata",
  class = "Mammalia",
  order = "Artiodactyla",
  family = "Bovidae",
  genus = "Ovis",
  species = "aries",
  characteristics = {
    intelligence: "Moderate",
    adaptability: "High",
    social_complexity: "High",
    wool_production: "High"
  },
  habitat = ["Grasslands", "Pastures", "Mountainous regions", "Domestic farms"],
  diet = ["Herbivorous", "Grasses", "Forbs", "Shrubs"],
  reproduction = "Sexual",
  lifespan = {
    average: "10-12 years",
    maximum: "20 years"
  },
  status = "Domesticated",
  related_species = ["species:human"];
  
-- Apple
CREATE species:apple SET
  name = "Apple",
  scientific_name = "Malus domestica",
  description = "Deciduous fruit tree producing edible fruits",
  kingdom = "Plantae",
  phylum = "Angiosperms",
  class = "Eudicots",
  order = "Rosales",
  family = "Rosaceae",
  genus = "Malus",
  species = "domestica",
  characteristics = {
    growth_rate: "Moderate",
    adaptability: "High",
    fruit_production: "High",
    hardiness: "High"
  },
  habitat = ["Temperate regions", "Orchards", "Gardens", "Wild forests"],
  diet = ["Photosynthetic", "Soil nutrients", "Water"],
  reproduction = "Sexual and Asexual",
  lifespan = {
    average: "50-100 years",
    maximum: "200 years"
  },
  status = "Cultivated",
  related_species = ["species:human"]; 