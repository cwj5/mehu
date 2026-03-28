import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { resolveCameraUpVector } from './cameraUp';

describe('resolveCameraUpVector', () => {
    it('uses requested up when not parallel to look direction', () => {
        const pos = new THREE.Vector3(0, 0, 5);
        const requested = new THREE.Vector3(0, 1, 0);
        const up = resolveCameraUpVector(pos, requested);

        expect(up.distanceTo(new THREE.Vector3(0, 1, 0))).toBeLessThan(1e-9);
    });

    it('falls back when requested up is parallel to look direction', () => {
        const pos = new THREE.Vector3(5, 0, 0);
        const requested = new THREE.Vector3(1, 0, 0);
        const up = resolveCameraUpVector(pos, requested);

        const lookDir = pos.clone().multiplyScalar(-1).normalize();
        expect(Math.abs(up.dot(lookDir))).toBeLessThan(0.995);
    });

    it('defaults to world up when no requested up is provided', () => {
        const pos = new THREE.Vector3(3, 4, 5);
        const up = resolveCameraUpVector(pos, null);

        expect(up.distanceTo(new THREE.Vector3(0, 1, 0))).toBeLessThan(1e-9);
    });
});
