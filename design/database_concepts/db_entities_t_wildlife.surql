USE NS project_universe;
USE DB entities;

DEFINE TABLE wildlife SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD wildlife_id     ON TABLE wildlife TYPE string;
DEFINE FIELD name            ON TABLE wildlife TYPE string;
DEFINE FIELD species         ON TABLE wildlife TYPE string;
DEFINE FIELD habitat         ON TABLE wildlife TYPE string;
DEFINE FIELD diet            ON TABLE wildlife TYPE string;

DEFINE INDEX unique_wildlife_id ON TABLE wildlife COLUMNS wildlife_id UNIQUE;

-- Example common livestock
CREATE wildlife:wildlife_001 SET
  wildlife_id = "wildlife_001",
  name = "Cow",
  species = "Bos taurus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_002 SET
  wildlife_id = "wildlife_002",
  name = "Chicken",
  species = "Gallus gallus domesticus",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_003 SET
  wildlife_id = "wildlife_003",
  name = "Pig",
  species = "Sus scrofa domesticus",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_004 SET
  wildlife_id = "wildlife_004",
  name = "Sheep",
  species = "Ovis aries",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_005 SET
  wildlife_id = "wildlife_005",
  name = "Goat",
  species = "Capra aegagrus hircus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_006 SET
  wildlife_id = "wildlife_006",
  name = "Horse",
  species = "Equus ferus caballus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_007 SET
  wildlife_id = "wildlife_007",
  name = "Duck",
  species = "Anas platyrhynchos domesticus",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_008 SET
  wildlife_id = "wildlife_008",
  name = "Turkey",
  species = "Meleagris gallopavo",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_009 SET
  wildlife_id = "wildlife_009",
  name = "Rabbit",
  species = "Oryctolagus cuniculus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_010 SET
  wildlife_id = "wildlife_010",
  name = "Llama",
  species = "Lama glama",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_011 SET
  wildlife_id = "wildlife_011",
  name = "Alpaca",
  species = "Vicugna pacos",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_012 SET
  wildlife_id = "wildlife_012",
  name = "Donkey",
  species = "Equus africanus asinus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_013 SET
  wildlife_id = "wildlife_013",
  name = "Buffalo",
  species = "Bubalus bubalis",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_014 SET
  wildlife_id = "wildlife_014",
  name = "Camel",
  species = "Camelus dromedarius",
  habitat = "Deserts",
  diet = "Herbivore";

CREATE wildlife:wildlife_015 SET
  wildlife_id = "wildlife_015",
  name = "Yak",
  species = "Bos grunniens",
  habitat = "Mountains",
  diet = "Herbivore";

CREATE wildlife:wildlife_016 SET
  wildlife_id = "wildlife_016",
  name = "Guinea Pig",
  species = "Cavia porcellus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_017 SET
  wildlife_id = "wildlife_017",
  name = "Quail",
  species = "Coturnix coturnix",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_018 SET
  wildlife_id = "wildlife_018",
  name = "Pigeon",
  species = "Columba livia domestica",
  habitat = "Farms",
  diet = "Omnivore";

CREATE wildlife:wildlife_019 SET
  wildlife_id = "wildlife_019",
  name = "Goose",
  species = "Anser anser domesticus",
  habitat = "Farms",
  diet = "Herbivore";

CREATE wildlife:wildlife_020 SET
  wildlife_id = "wildlife_020",
  name = "Ostrich",
  species = "Struthio camelus",
  habitat = "Farms",
  diet = "Omnivore";
