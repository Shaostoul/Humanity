USE NS project_universe;
USE DB game;

DEFINE TABLE profiles SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD user_id         ON TABLE profiles TYPE record(users);
DEFINE FIELD display_name    ON TABLE profiles TYPE string;
DEFINE FIELD bio             ON TABLE profiles TYPE string;
DEFINE FIELD avatar_url      ON TABLE profiles TYPE string;
DEFINE FIELD preferences     ON TABLE profiles TYPE object;

DEFINE INDEX unique_user_id ON TABLE profiles COLUMNS user_id UNIQUE;

-- Example profiles
CREATE profiles:profile_001 SET
  user_id = users:john_doe,
  display_name = "John",
  bio = "Gamer and adventurer",
  avatar_url = "http://example.com/avatar/john_doe.png",
  preferences = {theme: "dark", notifications: true};

CREATE profiles:profile_002 SET
  user_id = users:jane_smith,
  display_name = "Jane",
  bio = "Admin and strategist",
  avatar_url = "http://example.com/avatar/jane_smith.png",
  preferences = {theme: "light", notifications: false};
