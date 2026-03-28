import * as THREE from 'three';

export type CameraPlotUpAxis =
    | 'positive_x'
    | 'positive_y'
    | 'positive_z'
    | 'negative_x'
    | 'negative_y'
    | 'negative_z';

export function resolveCameraUpVector(
    cameraPosition: THREE.Vector3,
    requestedUp: THREE.Vector3 | null,
): THREE.Vector3 {
    const defaultUp = new THREE.Vector3(0, 1, 0);
    const upCandidate = (requestedUp ?? defaultUp).clone().normalize();

    const posLenSq = cameraPosition.lengthSq();
    if (posLenSq < 1e-12) {
        return upCandidate;
    }

    // Camera always looks at origin in Viewer3D camera sync.
    const lookDir = cameraPosition.clone().multiplyScalar(-1).normalize();
    if (Math.abs(upCandidate.dot(lookDir)) < 0.995) {
        return upCandidate;
    }

    const fallbacks = [
        defaultUp,
        new THREE.Vector3(0, 0, 1),
        new THREE.Vector3(1, 0, 0),
    ];
    for (const fb of fallbacks) {
        const norm = fb.clone().normalize();
        if (Math.abs(norm.dot(lookDir)) < 0.995) {
            return norm;
        }
    }

    return defaultUp;
}
