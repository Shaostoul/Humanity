/* shared/defaults.js, Single source of truth for all settings defaults.
   Loaded before settings.js and settings-app.js via <script> tag. */
(function () {
  'use strict';
  window.HOS_STORAGE_KEY = 'humanity_settings';
  window.HOS_DEFAULTS = {
    // Core UI
    // Mirrors the native accent token in data/gui/theme.ron (--accent in the
    // generated theme.css). Keep in sync if theme.ron's accent changes.
    accent: '#ed8c24',
    theme: 'dark',
    fontSize: 'medium',
    fontSizePx: 16,
    // Theme customizer
    iconWeight: 3,
    iconSize: 20,
    borderRadius: 8,
    contentWidth: 0,
    lineHeight: 1.6,
    spacingScale: 100,
    // Color overrides
    successColor: '',
    dangerColor: '',
    warningColor: '',
    // Space background (faint galactic-core image behind every page)
    'space-bg': true,
    // Audio/chat
    soundEnabled: true,
    timestampMode: 'relative',
    // Layout
    compact: false,
    'font-size': 'medium',
    // Navigation
    'rgb-nav': true,
    'nav-tips': true,
    // Notifications
    'notif-dm': true,
    'notif-group': true,
    'notif-mention': true,
    'notif-quests': true,
    'notif-cal': true,
    'notif-sound': false,
    // Privacy
    'who-dm': 'everyone',
    'show-online': true,
    'read-receipts': true,
    'discoverable': true,
    'local-only': true,
    analytics: false,
    // Chat
    'msg-preview': true,
    'enter-send': true,
    timestamps: 'hover',
    'msg-group': true,
    'relay-url': '',
    // App
    language: 'en',
    'date-fmt': 'mdy',
    'time-fmt': '12h',
    'launch-chat': false,
    autosave: '60',
    // Account
    'display-name': '',
    'debug-panel': false,
    'verbose-log': false,
    // Accessibility
    'reduce-motion': false,
    'no-rgb': false,
    'high-contrast': false,
    'colorblind': 'none',
    'focus-ring': false,
    'dyslexia-font': false,
    'aria-enhanced': false,
    // Audio/Video devices
    'mic-device': '',
    'mic-gain': '100',
    'speaker-device': '',
    'speaker-vol': '100',
    'camera-device': '',
    'video-quality': '720',
    // Security
    'auto-lock': '30',
    // Presence
    presence: 'online',
    'status-text': '',
    'quiet-hours': false,
    'quiet-start': '22:00',
    'quiet-end': '08:00',
    'dnd-friends': true,
    'dnd-mentions': false,
  };
})();
