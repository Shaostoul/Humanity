USE NS project_universe;
USE DB entities;

DEFINE TABLE stellar_objects SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD object_id       ON TABLE stellar_objects TYPE string;
DEFINE FIELD name            ON TABLE stellar_objects TYPE string;
DEFINE FIELD type            ON TABLE stellar_objects TYPE string;
DEFINE FIELD mass            ON TABLE stellar_objects TYPE string;
DEFINE FIELD diameter        ON TABLE stellar_objects TYPE string;
DEFINE FIELD distance_from_sun ON TABLE stellar_objects TYPE string;

DEFINE INDEX unique_object_id ON TABLE stellar_objects COLUMNS object_id UNIQUE;

-- Example stellar objects in our solar system
CREATE stellar_objects:object_001 SET
  object_id = "object_001",
  name = "Sun",
  type = "Star",
  mass = "1.989 × 10^30 kg",
  diameter = "1,391,000 km",
  distance_from_sun = "0 km";

CREATE stellar_objects:object_002 SET
  object_id = "object_002",
  name = "Earth",
  type = "Planet",
  mass = "5.972 × 10^24 kg",
  diameter = "12,742 km",
  distance_from_sun = "149.6 million km";
