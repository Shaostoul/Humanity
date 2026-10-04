PRAGMA foreign_keys=OFF;
BEGIN;
CREATE TABLE messages (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                msg_type  TEXT NOT NULL,
                from_key  TEXT,
                from_name TEXT,
                content   TEXT,
                timestamp INTEGER NOT NULL,
                signature TEXT,
                raw_json  TEXT NOT NULL
            , channel_id TEXT DEFAULT 'general', reply_to_from TEXT DEFAULT NULL, reply_to_timestamp INTEGER DEFAULT NULL, origin_server TEXT DEFAULT NULL);
CREATE TABLE peers (
                public_key  TEXT PRIMARY KEY,
                display_name TEXT,
                last_seen   INTEGER NOT NULL
            );
CREATE TABLE registered_names (
                name        TEXT NOT NULL COLLATE NOCASE,
                public_key  TEXT NOT NULL,
                kyber_public TEXT DEFAULT NULL,
                registered_at INTEGER NOT NULL, label TEXT DEFAULT NULL,
                PRIMARY KEY (name, public_key)
            );
CREATE TABLE link_codes (
                code        TEXT PRIMARY KEY,
                name        TEXT NOT NULL COLLATE NOCASE,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL
            );
CREATE TABLE channels (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                created_by  TEXT,
                created_at  INTEGER NOT NULL,
                read_only   INTEGER DEFAULT 0,
                local_only  INTEGER DEFAULT 0
            , position INTEGER DEFAULT 100, category_id INTEGER DEFAULT NULL, federated INTEGER DEFAULT 0, voice_enabled INTEGER DEFAULT 1);
CREATE TABLE invite_codes (
                code        TEXT PRIMARY KEY,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                used_by     TEXT
            );
CREATE TABLE user_roles (
                public_key  TEXT PRIMARY KEY,
                role        TEXT NOT NULL DEFAULT 'user'
            );
CREATE TABLE banned_keys (
                public_key  TEXT PRIMARY KEY,
                banned_at   INTEGER NOT NULL,
                name        TEXT NOT NULL DEFAULT ''
            );
CREATE TABLE muted_members (
                public_key  TEXT PRIMARY KEY,
                muted_at    INTEGER NOT NULL,
                name        TEXT NOT NULL DEFAULT ''
            );
CREATE TABLE user_uploads (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                filename    TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL,
                -- Shared-file library (v0.675): shared=1 files are publicly
                -- LISTED via GET /api/uploads and exempt from the per-user
                -- media FIFO (a shared .blend must not vanish because its
                -- uploader posted four chat photos). original_name keeps the
                -- human filename (the stored name is timestamp-mangled).
                shared        INTEGER NOT NULL DEFAULT 0,
                original_name TEXT NOT NULL DEFAULT '',
                size_bytes    INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE reports (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                reporter_key TEXT NOT NULL,
                reported_name TEXT NOT NULL,
                reason      TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL
            );
CREATE TABLE reactions (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                target_from     TEXT NOT NULL,
                target_timestamp INTEGER NOT NULL,
                emoji           TEXT NOT NULL,
                reactor_key     TEXT NOT NULL,
                reactor_name    TEXT NOT NULL DEFAULT '',
                channel         TEXT NOT NULL DEFAULT 'general',
                created_at      INTEGER NOT NULL,
                UNIQUE(target_from, target_timestamp, emoji, reactor_key)
            );
CREATE TABLE pinned_messages (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                channel           TEXT NOT NULL,
                from_key          TEXT NOT NULL,
                from_name         TEXT NOT NULL,
                content           TEXT NOT NULL,
                original_timestamp INTEGER NOT NULL,
                pinned_by         TEXT NOT NULL,
                pinned_at         INTEGER NOT NULL,
                UNIQUE(channel, from_key, original_timestamp)
            );
CREATE TABLE server_state (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
CREATE TABLE game_world_snapshots (
                world_id        TEXT PRIMARY KEY,
                snapshot_json   TEXT NOT NULL,
                game_time       REAL NOT NULL DEFAULT 0,
                next_entity_id  INTEGER NOT NULL DEFAULT 1,
                updated_at      INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE player_progress (
                public_key       TEXT PRIMARY KEY,
                current_quest    TEXT,
                completed_quests TEXT NOT NULL DEFAULT '[]',
                xp               INTEGER NOT NULL DEFAULT 0,
                reputation       INTEGER NOT NULL DEFAULT 0,
                updated_at       INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE game_banned_keys (
                public_key   TEXT NOT NULL,
                character_id TEXT,
                reason       TEXT NOT NULL DEFAULT '',
                banned_by    TEXT NOT NULL DEFAULT '',
                banned_at    INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (public_key, character_id)
            );
CREATE TABLE game_plots (
                world_id    TEXT NOT NULL,
                plot_id     TEXT NOT NULL,
                owner_did   TEXT NOT NULL,
                assigned_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (world_id, plot_id),
                UNIQUE (world_id, owner_did)
            );
CREATE TABLE federated_servers (
                server_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                public_key TEXT,
                trust_tier INTEGER NOT NULL DEFAULT 0,
                accord_compliant INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'unknown',
                last_seen INTEGER,
                added_at INTEGER NOT NULL
            );
CREATE TABLE dm_mailbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                to_key TEXT NOT NULL,
                content TEXT NOT NULL,
                received_day INTEGER NOT NULL
            );
CREATE TABLE user_data (
                public_key TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                updated_at INTEGER NOT NULL
            );
CREATE TABLE user_status (
                name TEXT PRIMARY KEY COLLATE NOCASE,
                status TEXT NOT NULL DEFAULT 'online',
                status_text TEXT NOT NULL DEFAULT ''
            );
CREATE TABLE profiles (
                name    TEXT PRIMARY KEY COLLATE NOCASE,
                bio     TEXT NOT NULL DEFAULT '',
                socials TEXT NOT NULL DEFAULT '{}'
            , avatar_url TEXT NOT NULL DEFAULT '', banner_url  TEXT NOT NULL DEFAULT '', pronouns    TEXT NOT NULL DEFAULT '', location    TEXT NOT NULL DEFAULT '', website     TEXT NOT NULL DEFAULT '', privacy     TEXT NOT NULL DEFAULT '{}');
CREATE TABLE channel_categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                collapsed INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE voice_channels (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                position    INTEGER DEFAULT 0,
                created_by  TEXT,
                created_at  INTEGER NOT NULL
            );
CREATE TABLE link_previews (
                url TEXT PRIMARY KEY,
                title TEXT,
                description TEXT,
                image TEXT,
                site_name TEXT,
                fetched_at INTEGER NOT NULL
            );
CREATE TABLE project_tasks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status      TEXT NOT NULL DEFAULT 'backlog',
                priority    TEXT NOT NULL DEFAULT 'medium',
                assignee    TEXT,
                created_by  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                position    INTEGER NOT NULL DEFAULT 0,
                labels      TEXT NOT NULL DEFAULT '[]'
            , project TEXT DEFAULT 'default');
CREATE TABLE task_comments (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id     INTEGER NOT NULL,
                author_key  TEXT NOT NULL,
                author_name TEXT NOT NULL,
                content     TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                FOREIGN KEY (task_id) REFERENCES project_tasks(id) ON DELETE CASCADE
            );
CREATE TABLE server_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                max_chars_unverified      INTEGER NOT NULL DEFAULT 280,
                max_chars_verified        INTEGER NOT NULL DEFAULT 1000,
                max_chars_mod             INTEGER NOT NULL DEFAULT 4000,
                max_chars_admin           INTEGER NOT NULL DEFAULT 10000,
                image_sharing_enabled     INTEGER NOT NULL DEFAULT 1,
                file_sharing_enabled      INTEGER NOT NULL DEFAULT 1,
                max_upload_mb             INTEGER NOT NULL DEFAULT 25,
                voice_channels_enabled    INTEGER NOT NULL DEFAULT 1,
                video_streaming_enabled   INTEGER NOT NULL DEFAULT 0,
                allowed_file_extensions   TEXT    NOT NULL DEFAULT 'png,jpg,jpeg,gif,webp,pdf,txt,md',
                max_uploads_per_user      INTEGER NOT NULL DEFAULT 4,
                max_total_upload_mb       INTEGER NOT NULL DEFAULT 500,
                max_uploads_per_user_unverified INTEGER NOT NULL DEFAULT 4,
                max_uploads_per_user_verified   INTEGER NOT NULL DEFAULT 20,
                max_uploads_per_user_mod        INTEGER NOT NULL DEFAULT 100,
                max_uploads_per_user_admin      INTEGER NOT NULL DEFAULT 500,
                require_pq_signatures           INTEGER NOT NULL DEFAULT 0,
                p2p_distribution_enabled        INTEGER NOT NULL DEFAULT 0,
                server_description        TEXT    NOT NULL DEFAULT '',
                server_name               TEXT    NOT NULL DEFAULT '',
                dm_mailbox_ttl_days       INTEGER NOT NULL DEFAULT 30,
                message_retention_days    INTEGER NOT NULL DEFAULT 0,
                erased_accounts_ttl_days  INTEGER NOT NULL DEFAULT 30,
                erased_accounts_cap       INTEGER NOT NULL DEFAULT 100000,
                world_time_scale          REAL    NOT NULL DEFAULT 72,
                updated_at                INTEGER NOT NULL DEFAULT 0,
                updated_by                TEXT
            , local_channel_enabled INTEGER NOT NULL DEFAULT 1, max_upload_mb_unverified INTEGER NOT NULL DEFAULT 5, max_upload_mb_verified   INTEGER NOT NULL DEFAULT 25, max_upload_mb_mod        INTEGER NOT NULL DEFAULT 100, max_upload_mb_admin      INTEGER NOT NULL DEFAULT 500);
CREATE TABLE erased_accounts (
                fingerprint TEXT PRIMARY KEY,
                erased_day  INTEGER NOT NULL,
                ttl_days    INTEGER NOT NULL
            ) WITHOUT ROWID;
CREATE TABLE roles (
                id          TEXT PRIMARY KEY,
                label       TEXT NOT NULL,
                color       TEXT NOT NULL,
                trust_level INTEGER NOT NULL,
                built_in    INTEGER NOT NULL,
                can_stream  INTEGER NOT NULL,
                can_upload  INTEGER NOT NULL,
                can_voice   INTEGER NOT NULL,
                base_tier   TEXT NOT NULL,
                sort_order  INTEGER NOT NULL DEFAULT 0,
                can_image_share INTEGER NOT NULL DEFAULT 1,
                can_file_share  INTEGER NOT NULL DEFAULT 1,
                max_chars        INTEGER NOT NULL DEFAULT 280,
                max_upload_mb    INTEGER NOT NULL DEFAULT 5,
                max_uploads_kept INTEGER NOT NULL DEFAULT 4
             );
CREATE TABLE friend_codes (
                code TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                uses_remaining INTEGER NOT NULL DEFAULT 1
            );
CREATE TABLE marketplace_listings (
                id TEXT PRIMARY KEY,
                seller_key TEXT NOT NULL,
                seller_name TEXT,
                title TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                condition TEXT,
                price TEXT,
                payment_methods TEXT,
                location TEXT,
                images TEXT,
                status TEXT DEFAULT 'active',
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT
            );
CREATE TABLE listing_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                listing_id TEXT NOT NULL,
                url TEXT NOT NULL,
                position INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (listing_id) REFERENCES marketplace_listings(id) ON DELETE CASCADE
            );
CREATE VIRTUAL TABLE marketplace_fts
            USING fts5(listing_id UNINDEXED, title, description, category);
CREATE TABLE listing_reviews (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                listing_id TEXT NOT NULL,
                reviewer_key TEXT NOT NULL,
                reviewer_name TEXT,
                rating INTEGER NOT NULL CHECK(rating >= 1 AND rating <= 5),
                comment TEXT DEFAULT '',
                created_at TEXT NOT NULL,
                FOREIGN KEY (listing_id) REFERENCES marketplace_listings(id) ON DELETE CASCADE,
                UNIQUE(listing_id, reviewer_key)
            );
CREATE TABLE seller_ratings (
                seller_key TEXT PRIMARY KEY,
                avg_rating REAL DEFAULT 0,
                review_count INTEGER DEFAULT 0
            );
CREATE TABLE assets (
                id          TEXT PRIMARY KEY,
                owner_key   TEXT NOT NULL,
                filename    TEXT NOT NULL,
                file_type   TEXT NOT NULL,
                category    TEXT NOT NULL,
                tags        TEXT DEFAULT '[]',
                size_bytes  INTEGER NOT NULL DEFAULT 0,
                url         TEXT NOT NULL,
                description TEXT DEFAULT '',
                uploaded_at TEXT DEFAULT (datetime('now'))
            );
CREATE TABLE streams (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                streamer_key TEXT NOT NULL,
                title        TEXT NOT NULL DEFAULT '',
                category     TEXT NOT NULL DEFAULT '',
                started_at   INTEGER NOT NULL,
                ended_at     INTEGER,
                viewer_peak  INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE stream_chat (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                stream_id  INTEGER NOT NULL,
                content    TEXT NOT NULL,
                from_name  TEXT NOT NULL DEFAULT '',
                source     TEXT NOT NULL DEFAULT 'humanity',
                timestamp  INTEGER NOT NULL,
                FOREIGN KEY (stream_id) REFERENCES streams(id) ON DELETE CASCADE
            );
CREATE TABLE user_skills (
                user_key    TEXT NOT NULL,
                skill_id    TEXT NOT NULL,
                reality_xp  REAL NOT NULL DEFAULT 0,
                fantasy_xp  REAL NOT NULL DEFAULT 0,
                level       INTEGER NOT NULL DEFAULT 0,
                updated_at  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (user_key, skill_id)
            );
CREATE TABLE skill_verifications (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id    TEXT NOT NULL,
                from_key    TEXT NOT NULL,
                to_key      TEXT NOT NULL,
                note        TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE push_subscriptions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                endpoint    TEXT NOT NULL UNIQUE,
                p256dh      TEXT NOT NULL,
                auth        TEXT NOT NULL,
                created_at  INTEGER NOT NULL
            );
CREATE TABLE notification_prefs (
                public_key        TEXT PRIMARY KEY,
                dm_enabled        INTEGER NOT NULL DEFAULT 1,
                mentions_enabled  INTEGER NOT NULL DEFAULT 1,
                tasks_enabled     INTEGER NOT NULL DEFAULT 1,
                dnd_start         TEXT DEFAULT NULL,
                dnd_end           TEXT DEFAULT NULL
            );
CREATE TABLE trades (
                id                TEXT PRIMARY KEY,
                initiator_key     TEXT NOT NULL,
                recipient_key     TEXT NOT NULL,
                status            TEXT NOT NULL DEFAULT 'pending',
                initiator_items   TEXT NOT NULL DEFAULT '[]',
                recipient_items   TEXT NOT NULL DEFAULT '[]',
                initiator_confirmed INTEGER DEFAULT 0,
                recipient_confirmed INTEGER DEFAULT 0,
                created_at        INTEGER NOT NULL,
                completed_at      INTEGER,
                message           TEXT
            );
CREATE TABLE trade_orders (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                seller_key      TEXT NOT NULL,
                item_type       TEXT NOT NULL,
                item_id         TEXT NOT NULL DEFAULT '',
                quantity         INTEGER NOT NULL,
                remaining_qty   INTEGER NOT NULL,
                price_per_unit  REAL NOT NULL,
                currency        TEXT NOT NULL DEFAULT 'credits',
                status          TEXT NOT NULL DEFAULT 'open',
                created_at      INTEGER NOT NULL,
                filled_at       INTEGER
            );
CREATE TABLE trade_history (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id        INTEGER NOT NULL,
                buyer_key       TEXT NOT NULL,
                seller_key      TEXT NOT NULL,
                item_type       TEXT NOT NULL,
                item_id         TEXT NOT NULL DEFAULT '',
                quantity         INTEGER NOT NULL,
                price_per_unit  REAL NOT NULL,
                total_price     REAL NOT NULL,
                timestamp       INTEGER NOT NULL
            );
CREATE TABLE key_rotations (
                old_key    TEXT PRIMARY KEY,
                new_key    TEXT NOT NULL,
                sig_by_old TEXT NOT NULL,
                sig_by_new TEXT NOT NULL,
                rotated_at INTEGER NOT NULL
            );
CREATE TABLE vault_blobs (
                public_key TEXT PRIMARY KEY,
                blob       TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
CREATE TABLE system_profiles (
                public_key TEXT PRIMARY KEY,
                profile    TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
CREATE TABLE projects (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT DEFAULT '',
                owner_key   TEXT NOT NULL,
                visibility  TEXT DEFAULT 'public',
                color       TEXT DEFAULT '#4488ff',
                icon        TEXT DEFAULT '📋',
                created_at  TEXT NOT NULL
            );
CREATE TABLE signed_profiles (
                public_key  TEXT PRIMARY KEY,
                name        TEXT NOT NULL DEFAULT '',
                bio         TEXT NOT NULL DEFAULT '',
                avatar_url  TEXT NOT NULL DEFAULT '',
                banner_url  TEXT NOT NULL DEFAULT '',
                socials     TEXT NOT NULL DEFAULT '{}',
                pronouns    TEXT NOT NULL DEFAULT '',
                location    TEXT NOT NULL DEFAULT '',
                website     TEXT NOT NULL DEFAULT '',
                timestamp   INTEGER NOT NULL DEFAULT 0,
                signature   TEXT NOT NULL DEFAULT ''
            );
CREATE TABLE signed_objects (
                object_id              TEXT PRIMARY KEY,
                protocol_version       INTEGER NOT NULL,
                object_type            TEXT NOT NULL,
                space_id               TEXT,
                channel_id             TEXT,
                author_fp              TEXT NOT NULL,
                author_pubkey          BLOB NOT NULL,
                created_at             INTEGER,
                payload_schema_version INTEGER NOT NULL,
                payload_encoding       TEXT NOT NULL,
                payload                BLOB NOT NULL,
                signature              BLOB NOT NULL,
                references_json        TEXT NOT NULL DEFAULT '[]',
                source_server          TEXT,
                received_at            INTEGER NOT NULL
            );
CREATE TABLE p2p_groups (
                group_id        TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                creator_fp      TEXT NOT NULL,
                creator_pubkey  BLOB NOT NULL,
                created_at      INTEGER,
                -- 1 once the creator publishes a group_disband_v1 — the group
                -- then disappears from every member's list (the disband object
                -- is the durable, P2P-replicable tombstone).
                disbanded       INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE p2p_group_roster (
                group_id      TEXT NOT NULL,
                member_fp     TEXT NOT NULL,
                member_pubkey BLOB NOT NULL,
                active        INTEGER NOT NULL DEFAULT 1,
                updated_at    INTEGER NOT NULL,
                PRIMARY KEY (group_id, member_fp)
            );
CREATE TABLE p2p_group_invites (
                invite_id    TEXT PRIMARY KEY,
                group_id     TEXT NOT NULL,
                secret_hash  BLOB NOT NULL,
                expires_at   INTEGER NOT NULL,
                created_at   INTEGER
            );
CREATE TABLE p2p_group_epochs (
                group_id    TEXT NOT NULL,
                epoch       INTEGER NOT NULL,
                object_id   TEXT NOT NULL,
                created_at  INTEGER,
                PRIMARY KEY (group_id, epoch)
            );
CREATE TABLE p2p_group_messages (
                object_id   TEXT PRIMARY KEY,
                group_id    TEXT NOT NULL,
                author_fp   TEXT NOT NULL,
                epoch       INTEGER NOT NULL,
                created_at  INTEGER NOT NULL
            );
CREATE TABLE vc_index (
                vc_object_id          TEXT PRIMARY KEY,
                issuer_did            TEXT NOT NULL,
                subject_did           TEXT NOT NULL,
                schema_id             TEXT NOT NULL,
                issued_at             INTEGER NOT NULL,
                expires_at            INTEGER,
                revoked_by_object_id  TEXT,
                withdrawn             INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE trust_scores (
                did                TEXT PRIMARY KEY,
                total              REAL NOT NULL,
                sub_scores_json    TEXT NOT NULL,
                inputs_json        TEXT NOT NULL,
                weights_version    INTEGER NOT NULL,
                computed_at        INTEGER NOT NULL
            );
CREATE TABLE proposals (
                proposal_object_id  TEXT PRIMARY KEY,
                proposer_did        TEXT NOT NULL,
                proposal_type       TEXT NOT NULL,
                scope               TEXT NOT NULL,
                space_id            TEXT,
                opens_at            INTEGER NOT NULL,
                closes_at           INTEGER NOT NULL,
                created_at          INTEGER NOT NULL,
                -- Scheduled re-votes (v0.1302). A decision is never mutated; it
                -- is SUPERSEDED by a later proposal on the same question, so the
                -- chain stays a real audit trail. All three are nullable and a
                -- proposal with all three NULL behaves exactly as before.
                --   review_after      absolute ms when this should be reviewed
                --   review_cadence_ms recurring interval, set the next review
                --   supersedes        proposal_object_id this one replaces
                review_after        INTEGER,
                review_cadence_ms   INTEGER,
                supersedes          TEXT
            );
CREATE TABLE votes (
                vote_object_id      TEXT PRIMARY KEY,
                proposal_object_id  TEXT NOT NULL,
                voter_did           TEXT NOT NULL,
                choice              TEXT NOT NULL,
                weight_at_vote      REAL NOT NULL,
                cast_at             INTEGER NOT NULL,
                UNIQUE(proposal_object_id, voter_did)
            );
CREATE TABLE ai_status (
                did             TEXT PRIMARY KEY,
                subject_class   TEXT NOT NULL,
                operator_did    TEXT,
                last_updated    INTEGER NOT NULL
            );
CREATE TABLE recovery_shares (
                share_object_id  TEXT PRIMARY KEY,
                holder_did       TEXT NOT NULL,
                guardian_did     TEXT NOT NULL,
                threshold        INTEGER NOT NULL,
                total_shares     INTEGER NOT NULL,
                created_at       INTEGER NOT NULL
            );
CREATE TABLE recovery_requests (
                request_object_id    TEXT PRIMARY KEY,
                holder_did           TEXT NOT NULL,
                new_pubkey           BLOB NOT NULL,
                threshold_required   INTEGER NOT NULL,
                approvals_count      INTEGER NOT NULL DEFAULT 0,
                status               TEXT NOT NULL DEFAULT 'open',
                created_at           INTEGER NOT NULL
            );
CREATE TABLE recovery_approvals (
                approval_object_id   TEXT PRIMARY KEY,
                request_object_id    TEXT NOT NULL,
                guardian_did         TEXT NOT NULL,
                submitted_at         INTEGER NOT NULL,
                UNIQUE(request_object_id, guardian_did)
            );
CREATE TABLE agent_sessions (
                scope_id            TEXT PRIMARY KEY,
                agent_id            TEXT NOT NULL,
                state               TEXT NOT NULL DEFAULT 'working',
                last_state_notes    TEXT NOT NULL DEFAULT '',
                claimed_at          INTEGER NOT NULL,
                last_heartbeat      INTEGER NOT NULL,
                completion_estimate REAL
            );
CREATE TABLE issuer_trust (
                observer_server  TEXT NOT NULL,
                issuer_did       TEXT NOT NULL,
                trust            REAL NOT NULL,
                good_count       INTEGER NOT NULL DEFAULT 0,
                bad_count        INTEGER NOT NULL DEFAULT 0,
                last_event_at    INTEGER NOT NULL,
                PRIMARY KEY (observer_server, issuer_did)
            );
CREATE TABLE server_members (
                public_key TEXT PRIMARY KEY,
                name       TEXT,
                role       TEXT NOT NULL DEFAULT 'member',
                joined_at  TEXT NOT NULL,
                last_seen  TEXT,
                hide_presence INTEGER NOT NULL DEFAULT 0
            , did_fp TEXT);
CREATE VIRTUAL TABLE messages_fts
            USING fts5(content, from_name, channel_id, content='messages', content_rowid='id');
CREATE TABLE guilds (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                description  TEXT NOT NULL DEFAULT '',
                owner_key    TEXT NOT NULL,
                icon         TEXT NOT NULL DEFAULT '',
                color        TEXT NOT NULL DEFAULT '#4488ff',
                created_at   TEXT NOT NULL,
                member_count INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE guild_members (
                guild_id    TEXT NOT NULL,
                public_key  TEXT NOT NULL,
                role        TEXT NOT NULL DEFAULT 'member',
                joined_at   TEXT NOT NULL,
                PRIMARY KEY (guild_id, public_key)
            );
CREATE TABLE guild_invites (
                id              TEXT PRIMARY KEY,
                guild_id        TEXT NOT NULL,
                created_by      TEXT NOT NULL,
                code            TEXT NOT NULL UNIQUE,
                uses_remaining  INTEGER NOT NULL DEFAULT 1,
                expires_at      INTEGER NOT NULL
            );
CREATE TABLE reputation (
                public_key  TEXT PRIMARY KEY,
                score       INTEGER NOT NULL DEFAULT 0,
                level       INTEGER NOT NULL DEFAULT 0,
                updated_at  INTEGER NOT NULL DEFAULT 0
            );
CREATE TABLE reputation_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                public_key  TEXT NOT NULL,
                event_type  TEXT NOT NULL,
                points      INTEGER NOT NULL,
                reason      TEXT NOT NULL DEFAULT '',
                created_at  INTEGER NOT NULL,
                source_key  TEXT NOT NULL DEFAULT ''
            );
CREATE TABLE bug_reports (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                title           TEXT NOT NULL,
                description     TEXT NOT NULL,
                steps           TEXT NOT NULL DEFAULT '',
                expected        TEXT NOT NULL DEFAULT '',
                actual          TEXT NOT NULL DEFAULT '',
                severity        TEXT NOT NULL DEFAULT 'medium',
                category        TEXT NOT NULL DEFAULT 'other',
                reporter_key    TEXT NOT NULL,
                reporter_name   TEXT NOT NULL DEFAULT '',
                browser_info    TEXT NOT NULL DEFAULT '',
                page_url        TEXT NOT NULL DEFAULT '',
                version         TEXT NOT NULL DEFAULT '',
                status          TEXT NOT NULL DEFAULT 'open',
                votes           INTEGER NOT NULL DEFAULT 0,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );
CREATE TABLE bug_votes (
                bug_id      INTEGER NOT NULL,
                voter_key   TEXT NOT NULL,
                voted_at    INTEGER NOT NULL,
                PRIMARY KEY (bug_id, voter_key),
                FOREIGN KEY (bug_id) REFERENCES bug_reports(id) ON DELETE CASCADE
            );
INSERT INTO "game_world_snapshots" ("world_id","snapshot_json","game_time","next_entity_id","updated_at") VALUES ('game_world_snapshot_v10','{"entities":{"3":{"entity_type":"vending_unit","position":[73.748024,1.0,53.49541],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"vending_unit in The Commons","interactable":true,"room_id":"commons","ship_built":true,"storage":false},"last_update":0.0},"2":{"entity_type":"bench_seating","position":[85.15197,1.0,57.200775],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"bench_seating in The Commons","interactable":true,"room_id":"commons","ship_built":true,"storage":false},"last_update":0.0},"5":{"entity_type":"harvest_bin","position":[85.15198,1.0,37.799225],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"harvest_bin in The Commons","interactable":true,"room_id":"commons","ship_built":true,"storage":true},"last_update":0.0},"17":{"entity_type":"botanist","position":[91.48889,1.0,39.809082],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Bringing fresh greens to the mess hall","chore":{"id":"deliver_greens","label":"Bringing fresh greens to the mess hall","meal":false,"remaining":15.0,"room_id":"mess-hall","state":"traveling","target":[92.0,1.0,26.0]},"chore_agent":true,"chores_done":16,"description":"Tending the Commons garden","dialog":["These tomatoes are six weeks ahead of schedule. Look at them.","Don''t touch the green tray. I''m trialing a new nutrient mix.","Lettuce harvest in three days. Tell the mess hall.","Plants do better when you talk to them. I''m not joking."],"eats":true,"greetings":["Welcome! Mind the spore filter by the towers.","Quiet, please. The seedlings are sensitive."],"home_meal_credit":0.0,"household":"crew","interactable":true,"meals_at_home":0,"meals_eaten":1,"meals_missed":0,"name":"Botanist Yara","next_meal_at":52016.39999999841,"npc_seq":4,"role":"botanist","room_id":"commons","ship_built":true,"wander":{"max_x":98.0,"max_z":74.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"14":{"entity_type":"medic","position":[90.0,1.0,43.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Holding the walk-in clinic hour","chore":{"id":"clinic_hour","label":"Holding the walk-in clinic hour","meal":false,"remaining":9.800189971923828,"room_id":"commons","state":"working","target":[90.0,1.0,43.0]},"chore_agent":true,"chores_done":13,"description":"Holding the clinic hour in the Commons","dialog":["If you''re hurt, sit down. If you''re not, don''t touch anything.","Medkits are in the cabinet. They are not snacks.","Eat at the mess hall. Skipped meals are half my patients.","I patched up worse than you yesterday. Stay still."],"eats":true,"greetings":["Clinic hour. State your symptom, briefly.","You look pale. Drink some water."],"home_meal_credit":0.0,"household":"crew","interactable":true,"meals_at_home":0,"meals_eaten":1,"meals_missed":0,"name":"Dr. Kel","next_meal_at":44208.00000000107,"npc_seq":1,"role":"medical_officer","room_id":"commons","ship_built":true,"wander":{"max_x":98.0,"max_z":74.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"16":{"entity_type":"maintenance_bot","position":[68.0,1.0,24.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Restocking the mess hall''s stores","chore":{"id":"restock_stores","label":"Restocking the mess hall''s stores","meal":false,"remaining":15.550135612487793,"room_id":"mess-hall","state":"working","target":[68.0,1.0,24.0]},"chore_agent":true,"chores_done":21,"description":"Autonomous bot keeping the mess hall stocked","dialog":["[CB-7] Stores reconciled. Variance: zero.","[CB-7] Tables wiped. Resuming patrol.","[CB-7] Crate 19-A misaligned. Correcting.","[CB-7] Greetings, citizen. Please do not block the serving counter."],"eats":false,"greetings":["[CB-7] Citizen detected. Logging entry.","[CB-7] Welcome to the mess hall. Please mind the trolley."],"household":"crew","interactable":true,"name":"CB-7","npc_seq":3,"role":"maintenance","room_id":"mess-hall","ship_built":true,"wander":{"max_x":94.0,"max_z":27.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"9":{"entity_type":"bench_seating","position":[80.0,1.0,26.4],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"bench_seating in Mess hall","interactable":true,"room_id":"mess-hall","ship_built":true,"storage":false},"last_update":0.0},"19":{"entity_type":"player","position":[68.0,1.7,22.0],"rotation":[0.0,0.0,0.0,1.0],"owner":"e11e00f0","components":{"completed_quests":[],"current_quest":{"complete":false,"description":"Visit every shared place aboard the ship, then find your way back to your own home.","home_plot":null,"id":"explore_ship","places":[{"id":"commons","name":"The Commons"},{"id":"street-1","name":"First Street"},{"id":"mess-hall","name":"Mess hall"}],"reward":{"message":"You know your way around the ship now, and your way home.","reputation":5,"xp":100},"title":"Find your bearings","total_rooms":3,"visited":["mess-hall"]},"health":100.0,"home_plot":null,"inventory":[],"meals_taken":1,"reputation":0,"stamina":100.0,"xp":0},"last_update":0.0},"8":{"entity_type":"dining_table","position":[82.4,1.0,24.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"dining_table in Mess hall","interactable":true,"room_id":"mess-hall","ship_built":true,"storage":false},"last_update":0.0},"15":{"entity_type":"engineer","position":[89.53122,1.0,44.582367],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Taking the air handlers'' readings","chore":{"id":"air_handler_readings","label":"Taking the air handlers'' readings","meal":false,"remaining":30.0,"room_id":"commons","state":"traveling","target":[90.0,1.0,61.0]},"chore_agent":true,"chores_done":13,"description":"Keeping the Commons'' air and water running","dialog":["The air handlers are humming. Don''t tap the vents.","If a panel is hot, I already know. Walk away.","The plumbing under First Street is fixed. Try it before you complain again.","We keep this ship alive on duct tape and hope. Mostly hope."],"eats":true,"greetings":["Keep your hands off the valves, please.","Welcome. The fans whine. That''s normal."],"home_meal_credit":0.0,"household":"crew","interactable":true,"meals_at_home":0,"meals_eaten":1,"meals_missed":0,"name":"Chief Tan","next_meal_at":49665.59999999936,"npc_seq":2,"role":"chief_engineer","room_id":"commons","ship_built":true,"wander":{"max_x":98.0,"max_z":74.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"1":{"entity_type":"notice_board","position":[92.2,1.0,47.5],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"notice_board in The Commons","interactable":true,"room_id":"commons","ship_built":true,"storage":false},"last_update":0.0},"4":{"entity_type":"tool_cabinet","position":[73.748024,1.0,41.50459],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"tool_cabinet in The Commons","interactable":true,"room_id":"commons","ship_built":true,"storage":true},"last_update":0.0},"13":{"entity_type":"navigator","position":[94.0,1.0,21.5],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Eating a meal in the mess hall","chore":{"id":"meal_navigator","label":"Eating a meal in the mess hall","meal":true,"remaining":2.6003217697143555,"room_id":"mess-hall","state":"working","target":[94.0,1.0,21.5]},"chore_agent":true,"chores_done":16,"description":"Off the helm, keeping the ship''s notices","dialog":["On a cruise this long the helm mostly flies itself. I keep the notices instead.","If you see a notice that is out of date, flag me.","First Street gets busy at meal times. Mind the corner by the Commons door.","I''d kill for a fresh atlas. Real paper. Stars don''t move that fast."],"eats":true,"greetings":["Welcome to the Commons. The notices are on the board.","Mind the cabling, citizen."],"home_meal_credit":0.0,"household":"crew","interactable":true,"meals_at_home":0,"meals_eaten":1,"meals_missed":0,"name":"Helm Officer Vex","next_meal_at":39229.200000000565,"npc_seq":0,"role":"navigator","room_id":"commons","ship_built":true,"wander":{"max_x":98.0,"max_z":74.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"7":{"entity_type":"locker","position":[67.0,1.0,140.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"locker in First Street","interactable":true,"room_id":"street-1","ship_built":true,"storage":true},"last_update":0.0},"6":{"entity_type":"bench_seating","position":[73.0,1.0,140.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"bench_seating in First Street","interactable":true,"room_id":"street-1","ship_built":true,"storage":false},"last_update":0.0},"10":{"entity_type":"notice_board","position":[77.6,1.0,24.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"notice_board in Mess hall","interactable":true,"room_id":"mess-hall","ship_built":true,"storage":false},"last_update":0.0},"18":{"entity_type":"crewmate","position":[92.0,1.0,56.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"activity":"Browsing the market stalls","chore":{"id":"market_browse","label":"Browsing the market stalls","meal":false,"remaining":1.9000935554504395,"room_id":"commons","state":"working","target":[92.0,1.0,56.0]},"chore_agent":true,"chores_done":11,"description":"Off shift in the mess hall","dialog":["Off-shift. Quiet hour. Whisper if you must.","Cycled through three novels this rotation. Got a recommendation?","The mess hall''s stew is better than it sounds.","Wake me at 06:00 ship-time. Not earlier."],"eats":true,"greetings":["Hey. Off-shift, but pull up a chair.","The stores are by the serving counter if you''re hungry."],"home_meal_credit":0.0,"household":"crew","interactable":true,"meals_at_home":0,"meals_eaten":1,"meals_missed":0,"name":"Crewmate Nia","next_meal_at":61059.59999999475,"npc_seq":5,"role":"off_duty","room_id":"mess-hall","ship_built":true,"wander":{"max_x":94.0,"max_z":27.0,"min_x":66.0,"min_z":21.0,"speed":0.4,"y":1.0}},"last_update":43199.99999999033},"12":{"entity_type":"food_store","position":[67.0,1.0,22.0],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"capacity":300.0,"description":"The mess hall''s stores: take a meal with the take_meal action","interactable":true,"meals":92.99999999995771,"name":"The mess hall''s stores","room_id":"mess-hall","ship_built":true,"storage":true,"store_id":"mess-hall-stores"},"last_update":0.0},"11":{"entity_type":"locker","position":[80.0,1.0,21.6],"rotation":[0.0,0.0,0.0,1.0],"owner":null,"components":{"description":"locker in Mess hall","interactable":true,"room_id":"mess-hall","ship_built":true,"storage":true},"last_update":0.0}},"next_entity_id":20,"game_time":43199.99999999033}',43199.99999999033,20,1791146973725);
INSERT INTO "player_progress" ("public_key","current_quest","completed_quests","xp","reputation","updated_at") VALUES ('e11e00f0','explore_ship','[]',0,0,1791146973722);
INSERT INTO "server_settings" ("id","max_chars_unverified","max_chars_verified","max_chars_mod","max_chars_admin","image_sharing_enabled","file_sharing_enabled","max_upload_mb","voice_channels_enabled","video_streaming_enabled","allowed_file_extensions","max_uploads_per_user","max_total_upload_mb","max_uploads_per_user_unverified","max_uploads_per_user_verified","max_uploads_per_user_mod","max_uploads_per_user_admin","require_pq_signatures","p2p_distribution_enabled","server_description","server_name","dm_mailbox_ttl_days","message_retention_days","erased_accounts_ttl_days","erased_accounts_cap","world_time_scale","updated_at","updated_by","local_channel_enabled","max_upload_mb_unverified","max_upload_mb_verified","max_upload_mb_mod","max_upload_mb_admin") VALUES (1,280,1000,4000,10000,1,1,5,1,0,'png,jpg,jpeg,gif,webp,pdf,txt,md',4,500,4,20,100,500,0,0,'','',30,7,30,100000,24,1791146973719,'fixture_admin',1,5,25,100,500);
INSERT INTO "roles" ("id","label","color","trust_level","built_in","can_stream","can_upload","can_voice","base_tier","sort_order","can_image_share","can_file_share","max_chars","max_upload_mb","max_uploads_kept") VALUES ('unverified','Unverified','#9E9E9E',0,1,0,0,0,'unverified',0,1,1,280,5,4);
INSERT INTO "roles" ("id","label","color","trust_level","built_in","can_stream","can_upload","can_voice","base_tier","sort_order","can_image_share","can_file_share","max_chars","max_upload_mb","max_uploads_kept") VALUES ('verified','Verified','#4FC3F7',1,1,0,1,1,'verified',1,1,1,1000,25,20);
INSERT INTO "roles" ("id","label","color","trust_level","built_in","can_stream","can_upload","can_voice","base_tier","sort_order","can_image_share","can_file_share","max_chars","max_upload_mb","max_uploads_kept") VALUES ('donor','Donor','#FFD54F',2,1,0,1,1,'verified',2,1,1,1000,25,20);
INSERT INTO "roles" ("id","label","color","trust_level","built_in","can_stream","can_upload","can_voice","base_tier","sort_order","can_image_share","can_file_share","max_chars","max_upload_mb","max_uploads_kept") VALUES ('mod','Moderator','#81C784',3,1,1,1,1,'mod',3,1,1,4000,100,100);
INSERT INTO "roles" ("id","label","color","trust_level","built_in","can_stream","can_upload","can_voice","base_tier","sort_order","can_image_share","can_file_share","max_chars","max_upload_mb","max_uploads_kept") VALUES ('admin','Admin','#E57373',4,1,1,1,1,'admin',4,1,1,10000,500,500);
INSERT INTO "projects" ("id","name","description","owner_key","visibility","color","icon","created_at") VALUES ('default','General','Default project','system','public','#4488ff','📋','2026-10-04 20:49:33');
CREATE INDEX idx_messages_timestamp
                ON messages(timestamp);
CREATE INDEX idx_registered_names_name
                ON registered_names(name COLLATE NOCASE);
CREATE INDEX idx_user_uploads_key
                ON user_uploads(public_key, id);
CREATE INDEX idx_reactions_target
                ON reactions(target_from, target_timestamp);
CREATE INDEX idx_reactions_channel
                ON reactions(channel);
CREATE INDEX idx_pinned_channel
                ON pinned_messages(channel);
CREATE INDEX idx_dm_mailbox_to
                ON dm_mailbox(to_key, id);
CREATE INDEX idx_dm_mailbox_day
                ON dm_mailbox(received_day);
CREATE INDEX idx_messages_channel ON messages(channel_id, id);
CREATE INDEX idx_user_uploads_shared
                 ON user_uploads(shared, id);
CREATE INDEX idx_project_tasks_status
                ON project_tasks(status);
CREATE INDEX idx_task_comments_task
                ON task_comments(task_id);
CREATE INDEX idx_messages_reply_to
                ON messages(reply_to_from, reply_to_timestamp);
CREATE INDEX idx_erased_accounts_day ON erased_accounts(erased_day);
CREATE INDEX idx_friend_codes_key
                ON friend_codes(public_key);
CREATE INDEX idx_marketplace_seller
                ON marketplace_listings(seller_key);
CREATE INDEX idx_marketplace_status
                ON marketplace_listings(status);
CREATE INDEX idx_marketplace_category
                ON marketplace_listings(category);
CREATE INDEX idx_listing_images_listing
                ON listing_images(listing_id, position);
CREATE INDEX idx_reviews_listing
                ON listing_reviews(listing_id);
CREATE INDEX idx_reviews_reviewer
                ON listing_reviews(reviewer_key);
CREATE INDEX idx_assets_owner ON assets(owner_key);
CREATE INDEX idx_assets_category ON assets(category);
CREATE INDEX idx_assets_file_type ON assets(file_type);
CREATE INDEX idx_streams_started
                ON streams(started_at);
CREATE INDEX idx_stream_chat_stream
                ON stream_chat(stream_id, timestamp);
CREATE INDEX idx_user_skills_skill
                ON user_skills(skill_id, level);
CREATE INDEX idx_skill_verifications_to
                ON skill_verifications(to_key, skill_id);
CREATE INDEX idx_push_subs_key
                ON push_subscriptions(public_key);
CREATE INDEX idx_trades_initiator ON trades(initiator_key);
CREATE INDEX idx_trades_recipient ON trades(recipient_key);
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trade_orders_item ON trade_orders(item_type, status);
CREATE INDEX idx_trade_orders_seller ON trade_orders(seller_key, status);
CREATE INDEX idx_trade_history_buyer ON trade_history(buyer_key);
CREATE INDEX idx_trade_history_seller ON trade_history(seller_key);
CREATE INDEX idx_trade_history_item ON trade_history(item_type);
CREATE INDEX idx_trade_history_order ON trade_history(order_id);
CREATE INDEX idx_projects_owner ON projects(owner_key);
CREATE INDEX idx_project_tasks_project ON project_tasks(project);
CREATE INDEX idx_signed_profiles_timestamp
                ON signed_profiles(timestamp);
CREATE INDEX idx_signed_objects_type_space
                ON signed_objects(object_type, space_id);
CREATE INDEX idx_signed_objects_author_fp
                ON signed_objects(author_fp);
CREATE INDEX idx_signed_objects_received_at
                ON signed_objects(received_at);
CREATE INDEX idx_p2p_roster_member
                ON p2p_group_roster(member_fp);
CREATE INDEX idx_p2p_invites_group
                ON p2p_group_invites(group_id);
CREATE INDEX idx_p2p_messages_group
                ON p2p_group_messages(group_id, created_at);
CREATE INDEX idx_vc_subject ON vc_index(subject_did);
CREATE INDEX idx_vc_issuer  ON vc_index(issuer_did);
CREATE INDEX idx_vc_schema  ON vc_index(schema_id);
CREATE INDEX idx_proposals_scope ON proposals(scope);
CREATE INDEX idx_proposals_type  ON proposals(proposal_type);
CREATE INDEX idx_proposals_space ON proposals(space_id);
CREATE INDEX idx_votes_proposal ON votes(proposal_object_id);
CREATE INDEX idx_votes_voter    ON votes(voter_did);
CREATE INDEX idx_recovery_holder   ON recovery_shares(holder_did);
CREATE INDEX idx_recovery_guardian ON recovery_shares(guardian_did);
CREATE INDEX idx_recovery_requests_holder ON recovery_requests(holder_did);
CREATE INDEX idx_recovery_requests_status ON recovery_requests(status);
CREATE INDEX idx_recovery_approvals_request
                ON recovery_approvals(request_object_id);
CREATE INDEX idx_agent_sessions_heartbeat
                ON agent_sessions(last_heartbeat);
CREATE INDEX idx_issuer_trust_did ON issuer_trust(issuer_did);
CREATE INDEX idx_proposals_supersedes ON proposals(supersedes);
CREATE INDEX idx_proposals_review ON proposals(review_after);
CREATE INDEX idx_server_members_did_fp ON server_members(did_fp);
CREATE TRIGGER messages_fts_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content, from_name, channel_id)
                VALUES (new.id, COALESCE(new.content,''), COALESCE(new.from_name,''), COALESCE(new.channel_id,'general'));
            END;
CREATE TRIGGER messages_fts_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, from_name, channel_id)
                VALUES ('delete', old.id, COALESCE(old.content,''), COALESCE(old.from_name,''), COALESCE(old.channel_id,'general'));
            END;
CREATE TRIGGER messages_fts_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, from_name, channel_id)
                VALUES ('delete', old.id, COALESCE(old.content,''), COALESCE(old.from_name,''), COALESCE(old.channel_id,'general'));
                INSERT INTO messages_fts(rowid, content, from_name, channel_id)
                VALUES (new.id, COALESCE(new.content,''), COALESCE(new.from_name,''), COALESCE(new.channel_id,'general'));
            END;
CREATE INDEX idx_guilds_owner ON guilds(owner_key);
CREATE INDEX idx_guild_members_key ON guild_members(public_key);
CREATE INDEX idx_guild_invites_code ON guild_invites(code);
CREATE INDEX idx_guild_invites_guild ON guild_invites(guild_id);
CREATE INDEX idx_reputation_events_key
                ON reputation_events(public_key, created_at);
CREATE INDEX idx_reputation_score
                ON reputation(score DESC);
CREATE INDEX idx_bug_reports_status ON bug_reports(status);
CREATE INDEX idx_bug_reports_severity ON bug_reports(severity);
CREATE INDEX idx_bug_reports_category ON bug_reports(category);
CREATE INDEX idx_bug_reports_reporter ON bug_reports(reporter_key);
COMMIT;
