USE NS project_universe;
USE DB game;

DEFINE TABLE sins SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD sin_name     ON TABLE sins TYPE string;
DEFINE FIELD description  ON TABLE sins TYPE string;

DEFINE INDEX unique_sin_name ON TABLE sins COLUMNS sin_name UNIQUE;

-- Example sins
CREATE sins:pride SET
  sin_name = "Pride",
  description = "Excessive belief in one's abilities, leading to vanity and arrogance.";

CREATE sins:greed SET
  sin_name = "Greed",
  description = "Intense and selfish desire for something, especially wealth or power.";

CREATE sins:lust SET
  sin_name = "Lust",
  description = "Intense longing, especially for sexual desires.";

CREATE sins:envy SET
  sin_name = "Envy",
  description = "Jealousy of others' traits, possessions, or status.";

CREATE sins:gluttony SET
  sin_name = "Gluttony",
  description = "Overindulgence and overconsumption of food, drink, or wealth items.";

CREATE sins:wrath SET
  sin_name = "Wrath",
  description = "Intense anger and hatred that can lead to violence and destruction.";

CREATE sins:sloth SET
  sin_name = "Sloth",
  description = "Laziness and failure to act and utilize one's talents."; 