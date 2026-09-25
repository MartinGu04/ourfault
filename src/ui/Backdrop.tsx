// The branded background: soft layered waves, a faint orbit and two glows,
// inspired by the OurFault welcome artwork. Colours come from theme tokens
// (see .backdrop in base.css); it is static and purely decorative.

export function Backdrop() {
  return (
    <div className="backdrop" aria-hidden="true">
      <svg viewBox="0 0 1440 900" preserveAspectRatio="xMidYMax slice" focusable="false">
        <defs>
          <linearGradient id="backdrop-wave-a" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" className="wave-a-from" />
            <stop offset="1" className="wave-a-to" />
          </linearGradient>
          <linearGradient id="backdrop-wave-b" x1="1" y1="0" x2="0" y2="1">
            <stop offset="0" className="wave-b-from" />
            <stop offset="1" className="wave-b-to" />
          </linearGradient>
        </defs>
        <circle className="wave-orbit" cx="1260" cy="90" r="430" />
        <circle className="wave-orbit" cx="1260" cy="90" r="560" />
        <path
          fill="url(#backdrop-wave-a)"
          d="M0 560C180 470 380 520 560 600S930 700 1110 610 1360 470 1440 450V900H0Z"
        />
        <path
          fill="url(#backdrop-wave-b)"
          d="M0 700C220 620 430 690 640 750S1040 800 1230 720 1400 640 1440 630V900H0Z"
        />
        <path className="wave-line" d="M0 612C200 530 390 575 570 650S930 745 1110 660 1360 520 1440 505" />
        <path className="wave-line" d="M0 742C230 668 440 732 650 790S1050 840 1240 760 1405 684 1440 676" />
      </svg>
    </div>
  );
}
