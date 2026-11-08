// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  CORE_SETTINGS_PATH,
  GLOBAL_SETTINGS_PATHS,
  globalSettingsPage,
  isGlobalSettingsPath,
} from './globalSettingsRoutes.ts';

describe('globalSettingsPage', () => {
  test('names the page of each Global Settings route', () => {
    assert.equal(
      globalSettingsPage('/dashboard/core-settings'),
      'core-settings'
    );
    assert.equal(globalSettingsPage('/dashboard/version'), 'version');
    assert.equal(globalSettingsPage('/dashboard/version/'), 'version');
  });

  test('is null for instance tabs and other routes', () => {
    for (const pathname of [
      '/',
      '/dashboard/overview',
      '/dashboard/settings',
      '/dashboard/core-settings/extra',
      '/dashboard/core-settingsx',
      '/settings/general',
      '/settings/version',
      '/core-settings',
    ]) {
      assert.equal(globalSettingsPage(pathname), null, pathname);
      assert.equal(isGlobalSettingsPath(pathname), false, pathname);
    }
  });

  test('the Core Settings link is a Global Settings route', () => {
    assert.equal(isGlobalSettingsPath(CORE_SETTINGS_PATH), true);
    for (const path of GLOBAL_SETTINGS_PATHS) {
      assert.equal(isGlobalSettingsPath(`/dashboard/${path}`), true);
    }
  });
});
