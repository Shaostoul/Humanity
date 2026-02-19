USE NS project_universe;
USE DB game;

DEFINE TABLE profiles SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD person_id     ON TABLE profiles TYPE record(people);
DEFINE FIELD display_name  ON TABLE profiles TYPE string;
DEFINE FIELD bio          ON TABLE profiles TYPE string;
DEFINE FIELD avatar_url   ON TABLE profiles TYPE string;
DEFINE FIELD preferences  ON TABLE profiles TYPE object;

DEFINE INDEX unique_person_id ON TABLE profiles COLUMNS person_id UNIQUE;

-- Example profiles
CREATE profiles:profile_001 SET
  person_id = people:john_doe,
  display_name = "John Doe",
  bio = "Adventurer and explorer of virtual worlds.",
  avatar_url = "https://example.com/avatars/john_doe.jpg",
  preferences = {
    theme: "dark",
    notifications: true,
    language: "en",
    timezone: "UTC"
  };

CREATE profiles:profile_002 SET
  person_id = people:jane_smith,
  display_name = "Jane Smith",
  bio = "Game developer and community moderator.",
  avatar_url = "https://example.com/avatars/jane_smith.jpg",
  preferences = {
    theme: "light",
    notifications: true,
    language: "en",
    timezone: "EST"
  }; 