'use client';

import { useEffect, useRef } from 'react';
import { gsap } from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import { useGSAP } from '@gsap/react';

gsap.registerPlugin(ScrollTrigger);

export default function GSAPAnimations() {
  const heroRef = useRef<HTMLDivElement>(null);

  useGSAP(() => {
    // Check for reduced motion preference
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (prefersReducedMotion) return;

    // Hero entrance animation
    const heroElements = [
      '.hero-meta',
      '.hero h1',
      '.hero-desc',
      '.hero-prompt-section',
      '.hero-or',
      '.cmd-block'
    ];

    gsap.from(heroElements, {
      opacity: 0,
      y: 20,
      duration: 0.8,
      stagger: 0.12,
      ease: 'power2.out',
      clearProps: 'all'
    });

    // Section headings scroll reveal
    gsap.utils.toArray('.sec-head').forEach((element) => {
      gsap.from(element as Element, {
        scrollTrigger: {
          trigger: element as Element,
          start: 'top 80%',
          toggleActions: 'play none none none'
        },
        opacity: 0,
        scale: 0.95,
        duration: 0.6,
        ease: 'power2.out'
      });
    });

    // Flow cards stagger
    gsap.from('.flow-card', {
      scrollTrigger: {
        trigger: '.flow',
        start: 'top 75%',
        toggleActions: 'play none none none'
      },
      opacity: 0,
      y: 30,
      duration: 0.6,
      stagger: 0.15,
      ease: 'power2.out'
    });

    // Bento cards with rotation
    gsap.from('.bento-card', {
      scrollTrigger: {
        trigger: '.bento',
        start: 'top 75%',
        toggleActions: 'play none none none'
      },
      opacity: 0,
      scale: 0.95,
      rotation: 2,
      duration: 0.6,
      stagger: 0.1,
      ease: 'power2.out'
    });

    // Reference grid items
    gsap.from('.ref-item', {
      scrollTrigger: {
        trigger: '.ref-grid',
        start: 'top 75%',
        toggleActions: 'play none none none'
      },
      opacity: 0,
      y: 20,
      duration: 0.5,
      stagger: 0.08,
      ease: 'power2.out'
    });

  }, { scope: heroRef });

  return <div ref={heroRef} style={{ display: 'contents' }} />;
}
