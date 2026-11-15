// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { instanceRedirect as redirect } from './instanceRoute.ts';

// on the hosted dashboard, served under /host/app
const instanceRedirect = (windowPathname: string, selected: boolean) =>
  redirect(windowPathname, selected, '/host/app');

describe('instanceRedirect under a base path', () => {
  test('stays on an instance tab', () => {
    assert.equal(instanceRedirect('/host/app/dashboard/console', true), null);
  });

  test('keeps the settings modal open', () => {
    assert.equal(instanceRedirect('/host/app/settings/general', true), null);
    assert.equal(instanceRedirect('/host/app/settings/general', false), null);
  });

  test('opens Overview when an instance is selected elsewhere', () => {
    assert.equal(instanceRedirect('/host/app', true), '/dashboard/overview');
    assert.equal(instanceRedirect('/host/app/', true), '/dashboard/overview');
  });

  test('goes home when the instance is gone', () => {
    assert.equal(instanceRedirect('/host/app/dashboard/files', false), '/');
    assert.equal(instanceRedirect('/host/app', false), null);
  });
});

describe('instanceRedirect without a base path', () => {
  test('reads the path as it is', () => {
    assert.equal(redirect('/dashboard/console', true, ''), null);
    assert.equal(redirect('/', true, ''), '/dashboard/overview');
    assert.equal(redirect('/dashboard/console', false, ''), '/');
  });
});
