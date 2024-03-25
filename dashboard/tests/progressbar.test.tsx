import React from 'react';
import { render } from '@testing-library/react';
import ProgressBar from '../src/components/Atoms/ProgressBar';

describe('ProgressBar Component', () => {
  it('renders progress bar with default height and color', () => {
    const { container } = render(<ProgressBar progress_percent={0.5} />);
    const progressBar = container.firstChild;

    expect(progressBar).toHaveClass('h-1'); // Ensure default height class is applied
    expect(progressBar?.firstChild).toHaveClass('bg-blue'); // Ensure default color class is applied
    expect(progressBar?.firstChild).toHaveStyle('width: 50%'); // Ensure width is set correctly
  });

  it('renders progress bar with custom height and color', () => {
    const { container } = render(<ProgressBar progress_percent={0.75} heightClass="h-2" colorClass="bg-green" />);
    const progressBar = container.firstChild;

    expect(progressBar).toHaveClass('h-2'); // Ensure custom height class is applied
    expect(progressBar?.firstChild).toHaveClass('bg-green'); // Ensure custom color class is applied
    expect(progressBar?.firstChild).toHaveStyle('width: 75%'); // Ensure width is set correctly
  });

  it('renders progress bar with 0% progress', () => {
    const { container } = render(<ProgressBar progress_percent={0} />);
    const progressBar = container.firstChild;

    expect(progressBar?.firstChild).toHaveStyle('width: 0%'); // Ensure width is set to 0%
  });

});
