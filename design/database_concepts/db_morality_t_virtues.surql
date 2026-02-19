USE NS project_universe;
USE DB game;

DEFINE TABLE virtues SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD virtue_name  ON TABLE virtues TYPE string;
DEFINE FIELD description  ON TABLE virtues TYPE string;

DEFINE INDEX unique_virtue_name ON TABLE virtues COLUMNS virtue_name UNIQUE;

-- Example virtues
CREATE virtues:humility SET
  virtue_name = "Humility",
  description = "Modest view of one's importance, avoiding pride and arrogance.";

CREATE virtues:charity SET
  virtue_name = "Charity",
  description = "Generosity and giving, especially to those in need.";

CREATE virtues:chastity SET
  virtue_name = "Chastity",
  description = "Purity of mind and body, moderation in all things.";

CREATE virtues:kindness SET
  virtue_name = "Kindness",
  description = "Compassion and consideration for others, countering envy.";

CREATE virtues:temperance SET
  virtue_name = "Temperance",
  description = "Self-control and moderation in all aspects of life.";

CREATE virtues:patience SET
  virtue_name = "Patience",
  description = "Ability to endure difficult situations without anger or frustration.";

CREATE virtues:diligence SET
  virtue_name = "Diligence",
  description = "Consistent and persistent effort in work and responsibilities."; 